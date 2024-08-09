use crate::{CaptureHandler, FrameArrived, Source, SourceType, VideoCaptureSourceDescription};

use std::{
    ptr::null_mut,
    sync::{atomic::AtomicBool, Arc, Mutex, RwLock},
    thread,
    time::Duration,
};

use anyhow::{anyhow, Result};
use frame::{VideoFrame, VideoSize, VideoTransform};
use utils::atomic::EasyAtomic;
use windows::{
    core::Interface,
    Win32::{Graphics::Direct3D11::ID3D11Texture2D, Media::MediaFoundation::IMF2DBuffer},
};

use windows_capture::{
    capture::{CaptureControl, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::{ColorFormat, CursorCaptureSettings, DrawBorderSettings, Settings},
};

struct WindowsCapture {
    texture: Arc<RwLock<Option<ID3D11Texture2D>>>,
    status: Arc<AtomicBool>,
}

impl GraphicsCaptureApiHandler for WindowsCapture {
    type Flags = (Box<dyn FrameArrived<Frame = VideoFrame>>, Context);
    type Error = anyhow::Error;

    fn new((mut arrived, ctx): Self::Flags) -> Result<Self, Self::Error> {
        let texture: Arc<RwLock<Option<ID3D11Texture2D>>> = Default::default();
        let status: Arc<AtomicBool> = Arc::new(AtomicBool::new(true));

        let mut frame = VideoFrame::default();
        frame.width = ctx.options.size.width;
        frame.height = ctx.options.size.height;

        let texture_ = Arc::downgrade(&texture);
        let status_ = status.clone();
        thread::Builder::new()
            .name("WindowsScreenCaptureTransformThread".to_string())
            .spawn(move || {
                let mut func = || {
                    while let Some(texture) = texture_.upgrade() {
                        if let Some(texture) = texture.read().unwrap().as_ref() {
                            if let Some(buffer) = ctx.transform.process(texture)? {
                                // If the buffer contains 2-D image data (such as an uncompressed
                                // video frame), you should query
                                // the buffer for the IMF2DBuffer
                                // interface. The methods on
                                // IMF2DBuffer are optimized for 2-D data.
                                let texture = buffer.cast::<IMF2DBuffer>()?;

                                // Gives the caller access to the memory in the buffer.
                                let mut stride = 0;
                                let mut data = null_mut();
                                unsafe { texture.Lock2D(&mut data, &mut stride)? };

                                frame.data[0] = data;
                                frame.data[1] =
                                    unsafe { data.add(stride as usize * frame.height as usize) };
                                frame.linesize = [stride as usize, stride as usize];
                                if !arrived.sink(&frame) {
                                    break;
                                }

                                // Unlocks a buffer that was previously locked.
                                unsafe { texture.Unlock2D()? };
                            }
                        }

                        thread::sleep(Duration::from_millis(1000 / ctx.options.fps as u64));
                    }

                    Ok::<(), anyhow::Error>(())
                };

                if let Err(e) = func() {
                    log::error!("WindowsScreenCaptureTransformThread error={:?}", e);
                } else {
                    log::info!("WindowsScreenCaptureTransformThread is closed");
                }

                status_.update(false);
            })?;

        Ok(Self { texture, status })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        if self.status.get() {
            // Video conversion always runs at a fixed frame rate. Here we simply update the
            // latest frame to effectively solve the frame rate mismatch problem.
            if let Ok(mut texture) = self.texture.write() {
                drop(texture.replace(frame.texture()?));
            }
        } else {
            log::info!("windows screen capture control stop");
            capture_control.stop();
        }

        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        self.status.update(false);
        Ok(())
    }
}

struct Context {
    transform: VideoTransform,
    options: VideoCaptureSourceDescription,
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
            .replace(WindowsCapture::start_free_threaded(Settings {
                cursor_capture: CursorCaptureSettings::WithoutCursor,
                draw_border: DrawBorderSettings::Default,
                color_format: ColorFormat::Bgra8,
                item: source,
                flags: (
                    Box::new(arrived),
                    Context {
                        transform: VideoTransform::new(
                            VideoSize {
                                width: source.width()?,
                                height: source.height()?,
                            },
                            options.fps,
                            VideoSize {
                                width: options.size.width,
                                height: options.size.height,
                            },
                            options.fps,
                        )?,
                        options,
                    },
                ),
            })?)
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
