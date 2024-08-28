use crate::{CaptureHandler, FrameArrived, Source, SourceType, VideoCaptureSourceDescription};

use std::{
    sync::{atomic::AtomicBool, Arc, Mutex},
    thread,
    time::Duration,
};

use anyhow::{anyhow, Result};
use frame::{Resource, VideoFormat, VideoFrame, VideoSize, VideoTransform, VideoTransformOptions};
use utils::{atomic::EasyAtomic, win32::MediaThreadClass};
use windows_capture::{
    capture::{CaptureControl, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::{ColorFormat, CursorCaptureSettings, Direct3D, DrawBorderSettings, Settings},
};

struct WindowsCapture {
    transform: Arc<Mutex<VideoTransform>>,
    status: Arc<AtomicBool>,
}

impl GraphicsCaptureApiHandler for WindowsCapture {
    type Flags = Context;
    type Error = anyhow::Error;

    fn new(mut ctx: Self::Flags) -> Result<Self, Self::Error> {
        let status: Arc<AtomicBool> = Arc::new(AtomicBool::new(true));
        let transform = Arc::new(Mutex::new(VideoTransform::new(VideoTransformOptions {
            direct3d: ctx.options.direct3d,
            input: Resource::Default(
                VideoFormat::RGBA,
                VideoSize {
                    width: ctx.source.width()?,
                    height: ctx.source.height()?,
                },
            ),
            output: Resource::Default(
                VideoFormat::NV12,
                VideoSize {
                    width: ctx.options.size.width,
                    height: ctx.options.size.height,
                },
            ),
        })?));

        let mut frame = VideoFrame::default();
        frame.width = ctx.options.size.width;
        frame.height = ctx.options.size.height;
        frame.hardware = false;

        let status_ = Arc::downgrade(&status);
        let transform_ = Arc::downgrade(&transform);
        thread::Builder::new()
            .name("WindowsScreenCaptureThread".to_string())
            .spawn(move || {
                let thread_class_guard = MediaThreadClass::Capture.join().ok();

                while let Some(transform) = transform_.upgrade() {
                    let mut transform = transform.lock().unwrap();
                    if let Err(e) = transform.process(None) {
                        log::error!("video transform process error={:?}", e);

                        break;
                    }

                    let buffer = if let Ok(buf) = transform.get_output_buffer() {
                        buf
                    } else {
                        break;
                    };

                    frame.linesize = [buffer.stride(), buffer.stride()];
                    frame.data[0] = buffer.buffer() as *const _;
                    frame.data[1] = unsafe {
                        buffer.buffer().add(buffer.stride() * frame.height as usize) as *const _
                    };

                    if !ctx.arrived.sink(&frame) {
                        break;
                    }

                    thread::sleep(Duration::from_millis(1000 / ctx.options.fps as u64));
                }

                if let Some(status) = status_.upgrade() {
                    status.update(false);
                }

                if let Some(guard) = thread_class_guard {
                    drop(guard)
                }
            })?;

        Ok(Self { status, transform })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        if self.status.get() {
            self.transform
                .lock()
                .unwrap()
                .update_input(frame.texture_ref());
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
    type CaptureOptions = VideoCaptureSourceDescription;

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
        options: Self::CaptureOptions,
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
                    options: options.clone(),
                    source,
                },
                Some(Direct3D {
                    device: options.direct3d.device,
                    context: options.direct3d.context,
                }),
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
