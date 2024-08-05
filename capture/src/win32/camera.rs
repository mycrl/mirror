use super::{
    IMFValue, MediaFoundation, MediaFoundationIMFAttributesSetHelper, MediaFoundationSourceType,
    SampleIterator,
};

use crate::{CaptureHandler, FrameArrived, Source, VideoCaptureSourceDescription};

use std::{
    ptr::null_mut,
    sync::{atomic::AtomicBool, Arc},
    thread,
};

use anyhow::{anyhow, Result};
use common::{atomic::EasyAtomic, frame::VideoFrame};
use windows::{
    core::Interface,
    Win32::Media::MediaFoundation::{
        IMF2DBuffer, IMFMediaSource, IMFSourceReader, MFCreateDeviceSource, MFCreateMediaType,
        MFCreateSourceReaderFromMediaSource, MFMediaType_Video, MFVideoFormat_NV12,
        MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
        MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, MF_MT_DEFAULT_STRIDE,
        MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE,
        MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING,
        MF_SOURCE_READER_FIRST_VIDEO_STREAM,
    },
};

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

        let buffer = unsafe { sample.ConvertToContiguousBuffer()? };
        let texture = buffer.cast::<IMF2DBuffer>()?;

        let mut stride = 0;
        let mut data = null_mut();
        unsafe {
            texture.Lock2D(&mut data, &mut stride)?;
        }

        let ret = {
            if !data.is_null() {
                self.frame.data[0] = data;
                self.frame.data[1] = unsafe { data.add(stride as usize * self.frame.rect.height) };
                self.frame.linesize = [stride as usize, stride as usize];
                self.arrived.sink(&self.frame)
            } else {
                false
            }
        };

        unsafe { texture.Unlock2D()? };
        if !ret {
            Err(anyhow!("failed to lock textture 2d"))
        } else {
            Ok(())
        }
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

pub struct CameraCapture(Arc<AtomicBool>);

impl CameraCapture {
    pub fn new() -> Result<Self> {
        log::info!("create camera capture");

        Ok(Self(Arc::new(AtomicBool::new(false))))
    }
}

impl CaptureHandler for CameraCapture {
    type Frame = VideoFrame;
    type Error = anyhow::Error;
    type CaptureOptions = VideoCaptureSourceDescription;

    fn get_sources(&self) -> Result<Vec<Source>, Self::Error> {
        MediaFoundation::get_sources(MediaFoundationSourceType::Video)
    }

    fn start<S: FrameArrived<Frame = Self::Frame> + 'static>(
        &self,
        opt: Self::CaptureOptions,
        arrived: S,
    ) -> Result<(), Self::Error> {
        let mut attributes = MediaFoundation::create_attributes()?;
        attributes.set(MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, IMFValue::U32(1))?;
        attributes.set(
            MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
            IMFValue::String(opt.source.id),
        )?;

        attributes.set(
            MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
            IMFValue::GUID(MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID),
        )?;

        attributes.set(
            MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING,
            IMFValue::U32(1),
        )?;

        // Creates an empty media type.
        let mut media_type = unsafe { MFCreateMediaType()? };
        media_type.set(MF_MT_MAJOR_TYPE, IMFValue::GUID(MFMediaType_Video))?;
        media_type.set(MF_MT_SUBTYPE, IMFValue::GUID(MFVideoFormat_NV12))?;
        media_type.set(MF_MT_DEFAULT_STRIDE, IMFValue::U32(opt.size.width))?;
        media_type.set(MF_MT_FRAME_RATE, IMFValue::DoubleU32(opt.fps as u32, 1))?;
        media_type.set(
            MF_MT_FRAME_SIZE,
            IMFValue::DoubleU32(opt.size.width, opt.size.height),
        )?;

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
        frame.rect.height = opt.size.height as usize;
        frame.rect.width = opt.size.width as usize;

        let mut ctx = Context {
            status: self.0.clone(),
            arrived,
            reader,
            device,
            frame,
        };

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
