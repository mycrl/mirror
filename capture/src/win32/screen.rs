use crate::{CaptureHandler, FrameArrived, Source, SourceType, VideoCaptureSourceDescription};

use std::{
    sync::{atomic::AtomicBool, Arc},
    thread,
    time::Duration,
};

use hylarana_common::{
    atomic::EasyAtomic,
    frame::{VideoFormat, VideoFrame, VideoSubFormat},
    win32::{EasyTexture, MediaThreadClass},
    Size,
};

use hylarana_resample::win32::{Resource, VideoResampler, VideoResamplerDescriptor};
use parking_lot::Mutex;
use thiserror::Error;
use windows::{
    core::Interface,
    Win32::Graphics::{
        Direct3D11::{
            ID3D11DeviceContext, ID3D11Texture2D, D3D11_RESOURCE_MISC_SHARED, D3D11_TEXTURE2D_DESC,
            D3D11_USAGE_DEFAULT,
        },
        Dxgi::Common::{DXGI_FORMAT_NV12, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC},
    },
};

use windows_capture::{
    capture::{CaptureControl, Context, GraphicsCaptureApiHandler},
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

struct Surface(ID3D11Texture2D);

unsafe impl Sync for Surface {}
unsafe impl Send for Surface {}

struct WindowsCapture {
    texture: ID3D11Texture2D,
    device_context: ID3D11DeviceContext,
    status: Arc<AtomicBool>,
}

impl GraphicsCaptureApiHandler for WindowsCapture {
    type Flags = CaptureContext;
    type Error = ScreenCaptureError;

    fn new(
        Context {
            mut flags,
            device,
            device_context,
        }: Context<Self::Flags>,
    ) -> Result<Self, Self::Error> {
        let status: Arc<AtomicBool> = Arc::new(AtomicBool::new(true));

        // Because windows-capture and this library implementation use different devices
        // and contexts, the problem needs to be solved with an intermediate texture,
        // for which a cross-device shared resource handle is created, then
        // windows-capture writes the frame to the intermediate texture, and the
        // following capture thread creates the texture view from this intermediate
        // texture as well The following capture thread also creates the texture view
        // from this intermediate texture.
        let (texture, surface) = {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: flags.source.width()?,
                Height: flags.source.height()?,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                BindFlags: 0,
                CPUAccessFlags: 0,
                Usage: D3D11_USAGE_DEFAULT,
                MiscFlags: D3D11_RESOURCE_MISC_SHARED.0 as u32,
            };

            let mut tex = None;
            unsafe {
                device.CreateTexture2D(&desc, None, Some(&mut tex))?;
            }

            let texture = tex.unwrap();

            // Use as input to VideoResampler by sharing resources across devices.
            let surface = flags
                .options
                .direct3d
                .open_shared_texture(texture.get_shared()?)?;

            (texture, Surface(surface))
        };

        let mut frame = VideoFrame::default();
        frame.width = flags.options.size.width;
        frame.height = flags.options.size.height;
        frame.format = VideoFormat::NV12;
        frame.sub_format = if flags.options.hardware {
            VideoSubFormat::D3D11
        } else {
            VideoSubFormat::SW
        };

        // Convert texture formats and scale sizes.
        let mut transform = VideoResampler::new(VideoResamplerDescriptor {
            direct3d: flags.options.direct3d,
            input: Resource::Default(
                DXGI_FORMAT_R8G8B8A8_UNORM,
                Size {
                    width: flags.source.width()?,
                    height: flags.source.height()?,
                },
            ),
            output: Resource::Default(
                DXGI_FORMAT_NV12,
                Size {
                    width: flags.options.size.width,
                    height: flags.options.size.height,
                },
            ),
        })?;

        let status_ = Arc::downgrade(&status);
        thread::Builder::new()
            .name("WindowsScreenCaptureThread".to_string())
            .spawn(move || {
                let thread_class_guard = MediaThreadClass::Capture.join().ok();

                let mut func = || {
                    loop {
                        let view = transform.create_input_view(&surface.0, 0)?;
                        transform.process(Some(view))?;

                        if frame.sub_format == VideoSubFormat::D3D11 {
                            frame.data[0] = transform.get_output().as_raw();
                            frame.data[1] = 0 as *const _;

                            if !flags.arrived.sink(&frame) {
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

                            if !flags.arrived.sink(&frame) {
                                break;
                            }
                        }

                        thread::sleep(Duration::from_millis(1000 / flags.options.fps as u64));
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
            device_context,
            status,
            texture,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        if self.status.get() {
            // Updates the texture in the frame to the middle texture.
            unsafe {
                self.device_context
                    .CopyResource(&self.texture, frame.as_raw_texture());
            }
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

struct CaptureContext {
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
                CaptureContext {
                    arrived: Box::new(arrived),
                    options,
                    source,
                },
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
