use super::{IMFValue, MediaFoundationIMFAttributesSetHelper};
use crate::{CaptureHandler, FrameArrived, Source, SourceType, VideoCaptureSourceDescription};

use std::{
    ptr::null_mut,
    slice::from_raw_parts,
    sync::{atomic::AtomicBool, Arc},
    thread,
};

use anyhow::{anyhow, Result};
use common::{atomic::EasyAtomic, frame::VideoFrame};
use windows::{
    core::Interface,
    Win32::Media::MediaFoundation::{
        IMF2DBuffer, IMFAttributes, IMFMediaSource, IMFSample, IMFSourceReader, MFCreateAttributes,
        MFCreateDeviceSource, MFCreateMediaType, MFCreateSourceReaderFromMediaSource,
        MFEnumDeviceSources, MFMediaType_Video, MFVideoFormat_NV12,
        MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
        MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
        MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, MF_MT_DEFAULT_STRIDE,
        MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE,
        MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING,
        MF_SOURCE_READER_FIRST_VIDEO_STREAM,
    },
};

/// Creates an empty attribute store.
fn create_attributes() -> Result<IMFAttributes> {
    let mut attributes = None;
    unsafe { MFCreateAttributes(&mut attributes, 1) }?;
    let attributes = attributes.ok_or_else(|| anyhow!("failed to create imf attributes"))?;
    Ok(attributes)
}

trait SampleIterator {
    type Item;

    fn next(&mut self) -> Result<Option<Self::Item>, anyhow::Error>;
}

impl SampleIterator for IMFSourceReader {
    type Item = IMFSample;

    fn next(&mut self) -> Result<Option<Self::Item>, anyhow::Error> {
        // Reads the next sample from the media source.
        let mut sample = None;
        let mut index = 0;
        let mut flags = 0;
        let mut timestamp = 0;
        unsafe {
            self.ReadSample(
                MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                0,
                Some(&mut index),
                Some(&mut flags),
                Some(&mut timestamp),
                Some(&mut sample),
            )?;
        }

        Ok(if index != 0 { None } else { sample })
    }
}

struct Context<T> {
    status: Arc<AtomicBool>,
    device: IMFMediaSource,
    reader: IMFSourceReader,
    frame: VideoFrame,
    arrived: T,
}

unsafe impl<T> Sync for Context<T> {}
unsafe impl<T> Send for Context<T> {}

impl<T: FrameArrived<Frame = VideoFrame>> Context<T> {
    fn poll(&mut self) -> Result<()> {
        if !self.status.get() {
            return Err(anyhow!("capture is stop"));
        }

        // Reads the next sample from the media source.
        let sample = if let Some(sample) = self.reader.next()? {
            sample
        } else {
            return Ok(());
        };

        if !self.status.get() {
            return Err(anyhow!("capture is stop"));
        }

        // Converts a sample with multiple buffers into a sample with a single buffer.
        let buffer = unsafe { sample.ConvertToContiguousBuffer()? };

        // If the buffer contains 2-D image data (such as an uncompressed video frame),
        // you should query the buffer for the IMF2DBuffer interface. The methods on
        // IMF2DBuffer are optimized for 2-D data.
        let texture = buffer.cast::<IMF2DBuffer>()?;

        // Gives the caller access to the memory in the buffer.
        let mut stride = 0;
        let mut data = null_mut();
        unsafe {
            texture.Lock2D(&mut data, &mut stride)?;
        }

        if data.is_null() {
            return Err(anyhow!("failed to lock textture 2d"));
        }

        self.frame.data[0] = data;
        self.frame.data[1] = unsafe { data.add(stride as usize * self.frame.height as usize) };
        self.frame.linesize = [stride as usize, stride as usize];
        if !self.arrived.sink(&self.frame) {
            return Err(anyhow!("FrameArrived sink return false"));
        }

        // Unlocks a buffer that was previously locked.
        unsafe { texture.Unlock2D()? };
        Ok(())
    }
}

impl<T> Drop for Context<T> {
    fn drop(&mut self) {
        self.status.update(false);

        // Stops all active streams in the media source.
        if let Err(e) = unsafe { self.device.Stop() } {
            log::warn!("camera capture device stop error={:?}", e);
        }
    }
}

#[derive(Default)]
pub struct CameraCapture(Arc<AtomicBool>);

impl CaptureHandler for CameraCapture {
    type Frame = VideoFrame;
    type Error = anyhow::Error;
    type CaptureOptions = VideoCaptureSourceDescription;

    fn get_sources() -> Result<Vec<Source>, Self::Error> {
        let mut attributes = create_attributes()?;
        attributes.set(
            MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
            IMFValue::GUID(MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID),
        )?;

        // Enumerates a list of audio or video capture devices.
        let mut count = 0;
        let mut activates = null_mut();
        unsafe {
            MFEnumDeviceSources(&attributes, &mut activates, &mut count)?;
        }

        if activates.is_null() {
            return Ok(Vec::new());
        }

        let mut sources = Vec::with_capacity(count as usize);
        for item in unsafe { from_raw_parts(activates, count as usize) } {
            if let Some(activate) = item {
                if let (Some(name), Some(id)) = (
                    activate.get_string(MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME),
                    activate.get_string(MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK),
                ) {
                    sources.push(Source {
                        id,
                        name,
                        index: sources.len(),
                        kind: SourceType::Camera,
                    });
                }
            }
        }

        Ok(sources)
    }

    #[rustfmt::skip]
    fn start<S: FrameArrived<Frame = Self::Frame> + 'static>(
        &self,
        opt: Self::CaptureOptions,
        arrived: S,
    ) -> Result<(), Self::Error> {
        let mut attributes = create_attributes()?;
        attributes.set(MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, IMFValue::U32(1))?;
        attributes.set(MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, IMFValue::String(opt.source.id))?;
        attributes.set(MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, IMFValue::GUID(MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID))?;
        attributes.set(MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, IMFValue::U32(1))?;

        // Creates a output media type.
        let mut media_type = unsafe { MFCreateMediaType()? };
        media_type.set(MF_MT_MAJOR_TYPE, IMFValue::GUID(MFMediaType_Video))?;
        media_type.set(MF_MT_SUBTYPE, IMFValue::GUID(MFVideoFormat_NV12))?;
        media_type.set(MF_MT_DEFAULT_STRIDE, IMFValue::U32(opt.size.width))?;
        media_type.set(MF_MT_FRAME_RATE, IMFValue::DoubleU32(opt.fps as u32, 1))?;
        media_type.set(MF_MT_FRAME_SIZE, IMFValue::DoubleU32(opt.size.width, opt.size.height))?;

        // Creates a media source for a hardware capture device.
        let device = unsafe { MFCreateDeviceSource(&attributes)? };

        // Creates the source reader from a media source.
        let reader = unsafe { MFCreateSourceReaderFromMediaSource(&device, &attributes)? };

        // Sets the media type for a stream.
        //
        // This media type defines that format that the Source Reader produces as
        // output. It can differ from the native format provided by the media source.
        unsafe {
            reader.SetCurrentMediaType(
                MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                None,
                &media_type,
            )?;
        }

        let mut frame = VideoFrame::default();
        frame.height = opt.size.height;
        frame.width = opt.size.width;

        let mut ctx = Context {
            status: self.0.clone(),
            arrived,
            reader,
            device,
            frame,
        };

        // Create a thread to continuously process the video frames read from the 
        // device and pass them to the receiver.
        self.0.update(true);
        thread::Builder::new()
            .name("WindowsCameraCaptureThread".to_string())
            .spawn(move || {
                loop {
                    if let Err(e) = ctx.poll() {
                        log::error!("WindowsCameraCaptureThread error={}", e);

                        break;
                    }
                }

                log::info!("WindowsCameraCaptureThread stop");
                ctx.status.update(false);
            })?;

        Ok(())
    }

    fn stop(&self) -> Result<(), Self::Error> {
        log::info!("stop camera capture");

        self.0.update(false);
        Ok(())
    }
}
