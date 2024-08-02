use super::{CaptureFrameHandler, CaptureHandler, Source, VideoCaptureSourceDescription};

use std::{
    ptr::null_mut,
    sync::{atomic::AtomicBool, Arc},
    thread,
};

use anyhow::{anyhow, Result};
use common::{atomic::EasyAtomic, frame::VideoFrame};
use windows::{
    core::{Interface, GUID, HSTRING, PCWSTR, PWSTR},
    Win32::{
        Media::MediaFoundation::{
            IMF2DBuffer, IMFActivate, IMFMediaSource, IMFSourceReader, MFCreateAttributes,
            MFCreateDeviceSource, MFCreateMediaType, MFCreateSourceReaderFromMediaSource,
            MFEnumDeviceSources, MFMediaType_Video, MFShutdown, MFStartup, MFVideoFormat_NV12,
            MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
            MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
            MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, MF_MT_DEFAULT_STRIDE,
            MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE,
            MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS,
            MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, MF_SOURCE_READER_FIRST_VIDEO_STREAM,
            MF_VERSION,
        },
        System::Com::CoTaskMemFree,
    },
};

pub struct CameraCapture(Arc<AtomicBool>);

impl CameraCapture {
    pub fn new() -> Result<Self> {
        log::info!("create camera capture");

        // Initializes Microsoft Media Foundation.
        unsafe {
            MFStartup(MF_VERSION, 0)?;
        }

        Ok(Self(Arc::new(AtomicBool::new(false))))
    }
}

impl Drop for CameraCapture {
    fn drop(&mut self) {
        let _ = self.stop();

        // Shuts down the Microsoft Media Foundation platform. Call this function once
        // for every call to MFStartup. Do not call this function from work queue
        // threads.
        if let Err(e) = unsafe { MFShutdown() } {
            log::error!("camera capture MFShutdown error={:?}", e);
        }
    }
}

impl CaptureHandler for CameraCapture {
    type Frame = VideoFrame;
    type Error = anyhow::Error;
    type CaptureOptions = VideoCaptureSourceDescription;

    fn get_sources(&self) -> Result<Vec<Source>, Self::Error> {
        // Creates an empty attribute store.
        let attributes = unsafe {
            let mut attributes = None;
            MFCreateAttributes(&mut attributes, 1)?;
            if let Some(attributes) = attributes {
                attributes
            } else {
                return Err(anyhow!("failed to create attributes"));
            }
        };

        unsafe {
            attributes.SetGUID(
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
            )?;
        }

        // Enumerates a list of audio or video capture devices.
        let activates = unsafe {
            let mut count = 0;
            let mut activates = null_mut();
            MFEnumDeviceSources(&attributes, &mut activates, &mut count)?;
            if !activates.is_null() {
                std::slice::from_raw_parts(activates, count as usize)
            } else {
                return Err(anyhow!("devices is empty"));
            }
        };

        // This is a convenience method for getting the device name or symbolic link
        // from a device.
        let get_activate_string = |activate: &IMFActivate, guid: GUID| {
            let mut size = 0;
            let mut wchar = PWSTR::null();
            unsafe {
                activate.GetAllocatedString(&guid, &mut wchar, &mut size)?;
            }

            if !wchar.is_null() {
                let str = unsafe { wchar.to_string()? };
                unsafe {
                    CoTaskMemFree(Some(wchar.0 as *const _));
                }

                Ok(str)
            } else {
                Err(anyhow!("get activate allocated string failed"))
            }
        };

        let mut items = Vec::with_capacity(activates.len());
        for item in activates {
            if let Some(activate) = item {
                if let Ok(name) =
                    get_activate_string(activate, MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME)
                {
                    if let Ok(id) = get_activate_string(
                        activate,
                        MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
                    ) {
                        items.push(Source {
                            index: items.len(),
                            name,
                            id,
                        });
                    }
                }
            }
        }

        Ok(items)
    }

    fn start<S: CaptureFrameHandler<Frame = Self::Frame> + 'static>(
        &self,
        opt: Self::CaptureOptions,
        sink: S,
    ) -> Result<(), Self::Error> {
        // Creates an empty attribute store.
        let attributes = unsafe {
            let mut attributes = None;
            MFCreateAttributes(&mut attributes, 1)?;
            if let Some(attributes) = attributes {
                attributes
            } else {
                return Err(anyhow!("failed to create attributes"));
            }
        };

        unsafe {
            attributes.SetGUID(
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
            )?;

            // Enables advanced video processing by the Source Reader, including color space
            // conversion, deinterlacing, video resizing, and frame-rate conversion.
            attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)?;

            // Enables the source reader or sink writer to use hardware-based Media
            // Foundation transforms (MFTs).
            attributes.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)?;
            attributes.SetString(
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
                PCWSTR(HSTRING::from(opt.source.id).as_ptr()),
            )?;
        }

        // Creates an empty media type.
        let media_type = unsafe { MFCreateMediaType()? };
        unsafe {
            media_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            media_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
            media_type.SetUINT32(&MF_MT_DEFAULT_STRIDE, opt.width)?;
            media_type.SetUINT64(&MF_MT_FRAME_RATE, pack_u32_to_u64(opt.fps as u32, 1))?;
            media_type.SetUINT64(&MF_MT_FRAME_SIZE, pack_u32_to_u64(opt.width, opt.height))?;
        }

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

        let mut ctx = Context {
            is_runing: self.0.clone(),
            reader,
            device,
        };

        self.0.update(true);
        thread::Builder::new()
            .name("WindowsCameraCaptureThread".to_string())
            .spawn(move || {
                let mut frame = VideoFrame::default();
                frame.rect.height = opt.height as usize;
                frame.rect.width = opt.width as usize;

                loop {
                    if let Err(e) = ctx.poll(&mut frame, &sink) {
                        log::warn!("WindowsCameraCaptureThread error={:?}", e);

                        break;
                    }
                }
            })?;

        Ok(())
    }

    fn stop(&self) -> Result<(), Self::Error> {
        self.0.update(false);
        Ok(())
    }
}

const fn pack_u32_to_u64(a: u32, b: u32) -> u64 {
    ((a as u64) << 32) | b as u64
}

struct Context {
    is_runing: Arc<AtomicBool>,
    device: IMFMediaSource,
    reader: IMFSourceReader,
}

unsafe impl Sync for Context {}
unsafe impl Send for Context {}

impl Context {
    fn poll<S>(&mut self, frame: &mut VideoFrame, sinker: &S) -> Result<()>
    where
        S: CaptureFrameHandler<Frame = VideoFrame>,
    {
        if !self.is_runing.get() {
            return Err(anyhow!("capture is stop"));
        }

        // Reads the next sample from the media source.
        let mut sample = None;
        let mut index = 0;
        unsafe {
            let mut flags = 0;
            let mut timestamp = 0;
            self.reader.ReadSample(
                MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                0,
                Some(&mut index),
                Some(&mut flags),
                Some(&mut timestamp),
                Some(&mut sample),
            )?;
        }

        if index != 0 {
            return Ok(());
        }

        if !self.is_runing.get() {
            return Err(anyhow!("capture is stop"));
        }

        let sample = if let Some(sample) = sample {
            sample
        } else {
            return Ok(());
        };

        // Converts a sample with multiple buffers into a sample with a single buffer.
        let buffer = unsafe { sample.ConvertToContiguousBuffer()? };
        let texture = buffer.cast::<IMF2DBuffer>()?;

        // Gives the caller access to the memory in the buffer.
        let mut stride = 0;
        let mut data = null_mut();
        unsafe {
            texture.Lock2D(&mut data, &mut stride)?;
        }

        if data.is_null() {
            return Err(anyhow!("texture is null"));
        }

        frame.linesize[0] = stride as usize;
        frame.linesize[1] = stride as usize;

        frame.data[0] = data;
        frame.data[1] = unsafe { data.add(stride as usize * frame.rect.height) };

        if !sinker.sink(&frame) {
            return Err(anyhow!("capture is stop"));
        }

        // Unlocks a buffer that was previously locked. Call this method once for each
        // call to IMF2DBuffer::Lock2D.
        unsafe {
            texture.Unlock2D()?;
        }

        Ok(())
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        self.is_runing.update(false);

        // Stops all active streams in the media source.
        if let Err(e) = unsafe { self.device.Stop() } {
            log::warn!("camera capture device stop error={:?}", e);
        }
    }
}
