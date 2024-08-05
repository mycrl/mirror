use super::{IMFValue, MediaFoundationIMFAttributesSetHelper};
use crate::{
    CaptureHandler, FrameArrived, Size, Source, SourceType, VideoCaptureSourceDescription,
};

use std::{
    marker::PhantomData,
    mem::ManuallyDrop,
    ptr::null_mut,
    sync::{atomic::AtomicBool, Arc, RwLock},
    thread,
    time::Duration,
};

use anyhow::{anyhow, Result};
use common::{atomic::EasyAtomic, frame::VideoFrame};
use windows::{
    core::Interface,
    Win32::{
        Graphics::Direct3D11::ID3D11Texture2D,
        Media::MediaFoundation::{
            CLSID_VideoProcessorMFT, IMF2DBuffer, IMFMediaBuffer, IMFTransform, MFCreate2DMediaBuffer, MFCreateDXGISurfaceBuffer, MFCreateMediaType, MFCreateSample, MFMediaType_Video, MFVideoFormat_NV12, MFVideoFormat_RGB32, MFVideoInterlace_Progressive, MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, MFT_MESSAGE_NOTIFY_END_OF_STREAM, MFT_OUTPUT_DATA_BUFFER, MFT_OUTPUT_STATUS_SAMPLE_READY, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE
        },
        System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
    },
};

use windows_capture::{
    capture::GraphicsCaptureApiHandler,
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::{ColorFormat, CursorCaptureSettings, DrawBorderSettings, Settings},
};

struct Transform {
    processor: IMFTransform,
    size: Size,
}

unsafe impl Send for Transform {}
unsafe impl Sync for Transform {}

impl Transform {
    fn new(input: Size, output: Size, fps: u8) -> Result<Self> {
        // Create and configure the Video Processor MFT.
        let processor: IMFTransform =
            unsafe { CoCreateInstance(&CLSID_VideoProcessorMFT, None, CLSCTX_INPROC_SERVER)? };

        // Configure the input type to be a D3D texture in RGB32 format.
        unsafe {
            processor.SetInputType(
                0,
                &{
                    let mut ty = MFCreateMediaType()?;
                    ty.set_values([
                        (MF_MT_MAJOR_TYPE, IMFValue::GUID(MFMediaType_Video)),
                        (MF_MT_SUBTYPE, IMFValue::GUID(MFVideoFormat_RGB32)),
                        (
                            MF_MT_INTERLACE_MODE,
                            IMFValue::U32(MFVideoInterlace_Progressive.0 as u32),
                        ),
                        (MF_MT_FRAME_RATE, IMFValue::DoubleU32(fps as u32, 1)),
                        (
                            MF_MT_FRAME_SIZE,
                            IMFValue::DoubleU32(input.width, input.height),
                        ),
                    ])?;

                    ty
                },
                0,
            )?
        };

        // Configure the output type to NV12 format.
        unsafe {
            processor.SetOutputType(
                0,
                &{
                    let mut ty = MFCreateMediaType()?;
                    ty.set_values([
                        (MF_MT_MAJOR_TYPE, IMFValue::GUID(MFMediaType_Video)),
                        (MF_MT_SUBTYPE, IMFValue::GUID(MFVideoFormat_NV12)),
                        (
                            MF_MT_INTERLACE_MODE,
                            IMFValue::U32(MFVideoInterlace_Progressive.0 as u32),
                        ),
                        (MF_MT_FRAME_RATE, IMFValue::DoubleU32(fps as u32, 1)),
                        (
                            MF_MT_FRAME_SIZE,
                            IMFValue::DoubleU32(output.width, output.height),
                        ),
                    ])?;

                    ty
                },
                0,
            )?
        };

        unsafe {
            processor.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
        }

        Ok(Self {
            processor,
            size: output,
        })
    }

    fn process(&self, texture: &ID3D11Texture2D) -> Result<Option<IMFMediaBuffer>> {
        let sample = unsafe { MFCreateSample()? };
        let buffer =
            unsafe { MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, texture, 0, false)? };
        unsafe { sample.AddBuffer(&buffer)? };
        let _ = unsafe { self.processor.ProcessInput(0, &sample, 0) };

        if unsafe { self.processor.GetOutputStatus()? } != MFT_OUTPUT_STATUS_SAMPLE_READY.0 as u32 {
            return Ok(None);
        }

        let mut status = 0;
        let mut buffers = [MFT_OUTPUT_DATA_BUFFER::default()];
        let buffer = unsafe { MFCreate2DMediaBuffer(self.size.width, self.size.height, MFVideoFormat_NV12.data1, false)? };
        let sample = unsafe { MFCreateSample()? };
        unsafe { sample.AddBuffer(&buffer)? };

        buffers[0].dwStreamID = 0;
        buffers[0].pSample = ManuallyDrop::new(Some(sample));
        unsafe { self.processor.ProcessOutput(0, &mut buffers, &mut status)? };
        Ok(Some(buffer))
    }
}

impl Drop for Transform {
    fn drop(&mut self) {
        unsafe {
            let _ = self.processor.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0);
        }
    }
}

struct WindowsCapture<T> {
    _arrived: PhantomData<T>,
    texture: Arc<RwLock<Option<ID3D11Texture2D>>>,
    ctx: Arc<Context>,
}

impl<T> GraphicsCaptureApiHandler for WindowsCapture<T>
where
    T: FrameArrived<Frame = VideoFrame> + 'static,
{
    type Flags = (T, Context);
    type Error = anyhow::Error;

    fn new((mut arrived, ctx): Self::Flags) -> Result<Self, Self::Error> {
        let texture: Arc<RwLock<Option<ID3D11Texture2D>>> = Default::default();
        let ctx = Arc::new(ctx);

        let mut frame = VideoFrame::default();
        frame.rect.width = ctx.options.size.width as usize;
        frame.rect.height = ctx.options.size.height as usize;

        let ctx_ = Arc::downgrade(&ctx);
        let texture_ = texture.clone();
        thread::Builder::new()
            .name("WindowsScreenCaptureTransformThread".to_string())
            .spawn(move || {
                while let Some(ctx) = ctx_.upgrade() {
                    if let Some(texture) = texture_.read().unwrap().as_ref() {
                        if let Ok(Some(buffer)) = ctx.transform.process(texture) {
                            let texture = buffer.cast::<IMF2DBuffer>().unwrap();
    
                            let mut stride = 0;
                            let mut data = null_mut();
                            unsafe { texture.Lock2D(&mut data, &mut stride) }.unwrap();
                            
                            frame.data[0] = data;
                            frame.data[1] = unsafe { data.add(stride as usize * frame.rect.height) };
                            frame.linesize = [stride as usize, stride as usize];
                            if !arrived.sink(&frame) {
                                break;
                            }
    
                            unsafe { texture.Unlock2D() }.unwrap();
                        }
                    }

                    thread::sleep(Duration::from_millis(1000 / ctx.options.fps as u64));
                }

                log::info!("WindowsScreenCaptureTransformThread is closed");
                if let Some(ctx) = ctx_.upgrade() {
                    ctx.status.update(false);
                }
            })?;

        Ok(Self {
            _arrived: PhantomData::default(),
            texture,
            ctx,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        if self.ctx.status.get() {
            if let Ok(mut texture) = self.texture.write() {
                drop(texture.replace(frame.texture()?));
            }
        } else {
            log::info!("windows screen capture control stop");
            capture_control.stop();
        }

        Ok(())
    }
}

struct Context {
    transform: Transform,
    status: Arc<AtomicBool>,
    options: VideoCaptureSourceDescription,
}

pub struct ScreenCapture(Arc<AtomicBool>);

impl ScreenCapture {
    pub fn new() -> Result<Self> {
        Ok(Self(Arc::new(AtomicBool::new(false))))
    }
}

impl CaptureHandler for ScreenCapture {
    type Frame = VideoFrame;
    type Error = anyhow::Error;
    type CaptureOptions = VideoCaptureSourceDescription;

    fn get_sources(&self) -> Result<Vec<Source>, Self::Error> {
        let mut displays = Vec::with_capacity(10);
        for item in Monitor::enumerate()? {
            displays.push(Source {
                name: item.name()?,
                index: item.index()? - 1,
                id: item.device_name()?,
                kind: SourceType::Screen,
            });
        }

        Ok(displays)
    }

    fn start<S: FrameArrived<Frame = Self::Frame> + 'static>(
        &self,
        options: Self::CaptureOptions,
        arrived: S,
    ) -> Result<(), Self::Error> {
        let source = Monitor::from_index(options.source.index)?;

        self.0.update(true);
        WindowsCapture::start_free_threaded(Settings {
            flags: (
                arrived,
                Context {
                    transform: Transform::new(
                        Size {
                            width: source.width()?,
                            height: source.height()?,
                        },
                        options.size,
                        options.fps,
                    )?,
                    status: self.0.clone(),
                    options,
                },
            ),
            cursor_capture: CursorCaptureSettings::WithoutCursor,
            draw_border: DrawBorderSettings::Default,
            color_format: ColorFormat::Rgba8,
            item: source,
        })?;

        Ok(())
    }

    fn stop(&self) -> Result<(), Self::Error> {
        self.0.update(false);
        Ok(())
    }
}
