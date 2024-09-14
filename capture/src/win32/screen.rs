use crate::{CaptureHandler, FrameArrived, Source, SourceType, VideoCaptureSourceDescription};

use std::{
    sync::{atomic::AtomicBool, Arc, Mutex},
    thread,
    time::Duration,
};

use anyhow::{anyhow, Result};
use frame::{
    Resource, VideoFormat, VideoFrame, VideoSize, VideoTransform, VideoTransformDescriptor,
};
use utils::{
    atomic::EasyAtomic,
    win32::{Interface, MediaThreadClass},
};

use windows::Win32::Graphics::{Direct3D11::ID3D11Texture2D, Dxgi::IDXGIResource};
use windows_capture::{
    capture::{CaptureControl, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::{ColorFormat, CursorCaptureSettings, DrawBorderSettings, Settings},
};

struct SharedResource(ID3D11Texture2D);

unsafe impl Sync for SharedResource {}
unsafe impl Send for SharedResource {}

struct WindowsCapture {
    shared_resource: Arc<Mutex<Option<SharedResource>>>,
    status: Arc<AtomicBool>,
}

impl GraphicsCaptureApiHandler for WindowsCapture {
    type Flags = Context;
    type Error = anyhow::Error;

    fn new(mut ctx: Self::Flags) -> Result<Self, Self::Error> {
        let status: Arc<AtomicBool> = Arc::new(AtomicBool::new(true));
        let shared_resource: Arc<Mutex<Option<SharedResource>>> = Default::default();

        let mut frame = VideoFrame::default();
        frame.width = ctx.options.size.width;
        frame.height = ctx.options.size.height;
        frame.hardware = ctx.options.hardware;
        frame.format = if ctx.options.hardware {
            VideoFormat::RGBA
        } else {
            VideoFormat::NV12
        };

        let mut transform = VideoTransform::new(VideoTransformDescriptor {
            direct3d: ctx.options.direct3d,
            input: Resource::Default(
                VideoFormat::RGBA,
                VideoSize {
                    width: ctx.source.width()?,
                    height: ctx.source.height()?,
                },
            ),
            output: Resource::Default(
                frame.format,
                VideoSize {
                    width: ctx.options.size.width,
                    height: ctx.options.size.height,
                },
            ),
        })?;

        let status_ = Arc::downgrade(&status);
        let shared_resource_ = Arc::downgrade(&shared_resource);
        thread::Builder::new()
            .name("WindowsScreenCaptureThread".to_string())
            .spawn(move || {
                let thread_class_guard = MediaThreadClass::Capture.join().ok();

                let mut func = || {
                    while let Some(shared_resource) = shared_resource_.upgrade() {
                        if let Some(resource) = shared_resource.lock().unwrap().take() {
                            let texture = transform.open_shared_texture(unsafe {
                                resource.0.cast::<IDXGIResource>()?.GetSharedHandle()?
                            })?;

                            let view = transform.create_input_view(&texture, 0)?;
                            transform.process(Some(view))?;
                        }

                        if frame.hardware {
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

                    Ok::<_, anyhow::Error>(())
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
                .unwrap()
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
pub struct ScreenCapture(Mutex<Option<CaptureControl<WindowsCapture, anyhow::Error>>>);

impl CaptureHandler for ScreenCapture {
    type Frame = VideoFrame;
    type Error = anyhow::Error;
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
            .ok_or_else(|| anyhow!("not found the source"))?;

        // Start capturing the screen. This runs in a free thread. If it runs in the
        // current thread, you will encounter problems with Winrt runtime
        // initialization.
        if let Some(control) = self
            .0
            .lock()
            .unwrap()
            .replace(WindowsCapture::start_free_threaded(Settings::new(
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
            ))?)
        {
            control.stop()?;
        }

        Ok(())
    }

    fn stop(&self) -> Result<(), Self::Error> {
        if let Some(control) = self.0.lock().unwrap().take() {
            control.stop()?;
        }

        Ok(())
    }
}
