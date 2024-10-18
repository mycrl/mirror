use crate::{CaptureHandler, FrameArrived, Source, SourceType, VideoCaptureSourceDescription};

use std::{
    sync::{atomic::AtomicBool, Arc},
    thread,
    time::Duration,
};

use mirror_common::{
    atomic::EasyAtomic,
    frame::{VideoFormat, VideoFrame, VideoSubFormat},
    win32::{EasyTexture, MediaThreadClass},
    Size,
};

use mirror_resample::win32::{Resource, VideoResampler, VideoResamplerDescriptor};
use parking_lot::Mutex;
use thiserror::Error;
use windows::{
    core::Interface,
    Win32::Graphics::{
        Direct3D11::ID3D11Texture2D,
        Dxgi::Common::{DXGI_FORMAT_NV12, DXGI_FORMAT_R8G8B8A8_UNORM},
    },
};

use windows_capture::{
    capture::{CaptureControl, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::{ColorFormat, CursorCaptureSettings, DrawBorderSettings, Settings},
};

#[derive(Debug, Error)]
pub enum ScreenCaptureError {
    #[error(transparent)]
    CreateThreadError(#[from] std::io::Error),
    #[error(transparent)]
    MonitorError(#[from] windows_capture::monitor::Error),
    #[error(transparent)]
    FrameError(#[from] windows_capture::frame::Error),
    #[error(transparent)]
    Win32Error(#[from] windows::core::Error),
    #[error("not found a screen source")]
    NotFoundScreenSource,
    #[error("capture control error")]
    CaptureControlError(String),
    #[error("start capture error")]
    StartCaptureError(String),
}

struct SharedResource(ID3D11Texture2D);

unsafe impl Sync for SharedResource {}
unsafe impl Send for SharedResource {}

struct WindowsCapture {
    shared_resource: Arc<Mutex<Option<SharedResource>>>,
    status: Arc<AtomicBool>,
}

impl GraphicsCaptureApiHandler for WindowsCapture {
    type Flags = Context;
    type Error = ScreenCaptureError;

    fn new(mut ctx: Self::Flags) -> Result<Self, Self::Error> {
        let status: Arc<AtomicBool> = Arc::new(AtomicBool::new(true));
        let shared_resource: Arc<Mutex<Option<SharedResource>>> = Default::default();

        let mut frame = VideoFrame::default();
        frame.width = ctx.options.size.width;
        frame.height = ctx.options.size.height;
        frame.format = VideoFormat::NV12;
        frame.sub_format = if ctx.options.hardware {
            VideoSubFormat::D3D11
        } else {
            VideoSubFormat::SW
        };

        let mut transform = VideoResampler::new(VideoResamplerDescriptor {
            direct3d: ctx.options.direct3d.clone(),
            input: Resource::Default(
                DXGI_FORMAT_R8G8B8A8_UNORM,
                Size {
                    width: ctx.source.width()?,
                    height: ctx.source.height()?,
                },
            ),
            output: Resource::Default(
                DXGI_FORMAT_NV12,
                Size {
                    width: ctx.options.size.width,
                    height: ctx.options.size.height,
                },
            ),
        })?;

        let direct3d = ctx.options.direct3d;
        let status_ = Arc::downgrade(&status);
        let shared_resource_ = Arc::downgrade(&shared_resource);
        thread::Builder::new()
            .name("WindowsScreenCaptureThread".to_string())
            .spawn(move || {
                let thread_class_guard = MediaThreadClass::Capture.join().ok();

                let mut func = || {
                    while let Some(shared_resource) = shared_resource_.upgrade() {
                        if let Some(resource) = shared_resource.lock().take() {
                            let texture = direct3d.open_shared_texture(resource.0.get_shared()?)?;
                            let view = transform.create_input_view(&texture, 0)?;
                            transform.process(Some(view))?;
                        }

                        if frame.sub_format == VideoSubFormat::D3D11 {
                            frame.data[0] = transform.get_output().as_raw();
                            frame.data[1] = 0 as *const _;

                            if !ctx.arrived.sink(&frame) {
                                break;
                            }
                        } else {
                            let texture = transform.get_output_buffer()?;
                            frame.data[0] = texture.buffer() as *const _;
                            frame.data[1] = unsafe {
                                texture
                                    .buffer()
                                    .add(frame.width as usize * frame.height as usize)
                            } as *const _;

                            frame.linesize[0] = texture.stride();
                            frame.linesize[1] = texture.stride();

                            if !ctx.arrived.sink(&frame) {
                                break;
                            }
                        }

                        thread::sleep(Duration::from_millis(1000 / ctx.options.fps as u64));
                    }

                    Ok::<_, ScreenCaptureError>(())
                };

                if let Err(e) = func() {
                    log::error!("WindowsScreenCaptureThread stop, error={:?}", e);
                } else {
                    log::info!("WindowsScreenCaptureThread stop");
                }

                if let Some(status) = status_.upgrade() {
                    status.update(false);
                }

                if let Some(guard) = thread_class_guard {
                    drop(guard)
                }
            })?;

        Ok(Self {
            shared_resource,
            status,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        if self.status.get() {
            self.shared_resource
                .lock()
                .replace(SharedResource(frame.texture()?));
        } else {
            log::info!("windows screen capture control stop");

            control.stop();
        }

        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        self.status.update(false);
        Ok(())
    }
}

struct Context {
    arrived: Box<dyn FrameArrived<Frame = VideoFrame>>,
    options: VideoCaptureSourceDescription,
    source: Monitor,
}

#[derive(Default)]
pub struct ScreenCapture(Mutex<Option<CaptureControl<WindowsCapture, ScreenCaptureError>>>);

impl CaptureHandler for ScreenCapture {
    type Frame = VideoFrame;
    type Error = ScreenCaptureError;
    type CaptureDescriptor = VideoCaptureSourceDescription;

    fn get_sources() -> Result<Vec<Source>, Self::Error> {
        let primary_name = Monitor::primary()?.name()?;

        let mut displays = Vec::with_capacity(10);
        for item in Monitor::enumerate()? {
            displays.push(Source {
                name: item.name()?,
                index: item.index()?,
                id: item.device_name()?,
                kind: SourceType::Screen,
                is_default: item.name()? == primary_name,
            });
        }

        Ok(displays)
    }

    fn start<S: FrameArrived<Frame = Self::Frame> + 'static>(
        &self,
        options: Self::CaptureDescriptor,
        arrived: S,
    ) -> Result<(), Self::Error> {
        let source = Monitor::enumerate()?
            .into_iter()
            .find(|it| it.name().ok() == Some(options.source.name.clone()))
            .ok_or_else(|| ScreenCaptureError::NotFoundScreenSource)?;

        // Start capturing the screen. This runs in a free thread. If it runs in the
        // current thread, you will encounter problems with Winrt runtime
        // initialization.
        if let Some(control) = self.0.lock().replace(
            WindowsCapture::start_free_threaded(Settings::new(
                source,
                CursorCaptureSettings::WithoutCursor,
                DrawBorderSettings::Default,
                ColorFormat::Rgba8,
                Context {
                    arrived: Box::new(arrived),
                    options,
                    source,
                },
                None,
            ))
            .map_err(|e| ScreenCaptureError::StartCaptureError(e.to_string()))?,
        ) {
            control
                .stop()
                .map_err(|e| ScreenCaptureError::CaptureControlError(e.to_string()))?;
        }

        Ok(())
    }

    fn stop(&self) -> Result<(), Self::Error> {
        if let Some(control) = self.0.lock().take() {
            control
                .stop()
                .map_err(|e| ScreenCaptureError::CaptureControlError(e.to_string()))?;
        }

        Ok(())
    }
}
