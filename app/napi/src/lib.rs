use std::{
    sync::{atomic::AtomicBool, mpsc::channel, Arc},
    thread,
};

use mirror::{
    AudioFrame, Capture, FrameSinker, Mirror, MirrorReceiver, MirrorReceiverDescriptor,
    MirrorSender, MirrorSenderDescriptor, Render, TransportDescriptor, VideoFrame,
    VideoRenderBackend,
};

use napi::{
    bindgen_prelude::Function,
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
    JsString, JsUnknown,
};

use napi_derive::napi;
use utils::atomic::EasyAtomic;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::{Fullscreen, Window, WindowId},
};

struct Logger(ThreadsafeFunction<String, JsUnknown, JsString, false>);

#[napi]
#[repr(usize)]
pub enum LogLevel {
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
    Trace = 5,
}

impl log::Log for Logger {
    fn flush(&self) {}
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    #[allow(unused_variables)]
    fn log(&self, record: &log::Record) {
        self.0.call(
            format!(
                "[{}] - ({}) - {}",
                record.level(),
                record.target(),
                record.args(),
            ),
            ThreadsafeFunctionCallMode::NonBlocking,
        );
    }
}

#[napi(ts_args_type = "callback: (message: string) => void")]
pub fn startup(callback: Function) -> napi::Result<()> {
    let func = || {
        log::set_boxed_logger(Box::new(Logger(
            callback
                .build_threadsafe_function::<String>()
                .build_callback(|ctx| ctx.env.create_string(&ctx.value))?,
        )))?;

        log::set_max_level(log::LevelFilter::Info);
        std::panic::set_hook(Box::new(|info| {
            log::error!(
                "pnaic: location={:?}, message={:?}",
                info.location(),
                info.payload().downcast_ref::<String>(),
            );
        }));

        mirror::startup()?;
        Ok::<_, anyhow::Error>(())
    };

    func().map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
pub fn shutdown() -> napi::Result<()> {
    mirror::shutdown().map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
#[derive(Debug, Clone, Copy)]
pub enum Backend {
    Dx11,
    Wgpu,
}

impl Into<VideoRenderBackend> for Backend {
    fn into(self) -> VideoRenderBackend {
        match self {
            Self::Dx11 => VideoRenderBackend::Dx11,
            Self::Wgpu => VideoRenderBackend::Wgpu,
        }
    }
}

#[napi(object)]
pub struct MirrorServiceDescriptor {
    pub server: String,
    pub multicast: String,
    pub mtu: u32,
}

impl TryInto<TransportDescriptor> for MirrorServiceDescriptor {
    type Error = napi::Error;

    fn try_into(self) -> Result<TransportDescriptor, Self::Error> {
        let func = || {
            Ok::<_, anyhow::Error>(TransportDescriptor {
                multicast: self.multicast.parse()?,
                server: self.server.parse()?,
                mtu: self.mtu as usize,
            })
        };

        func().map_err(|e| napi::Error::from_reason(e.to_string()))
    }
}

#[napi]
#[derive(Debug, Clone, Copy)]
pub enum VideoDecoderType {
    D3D11,
    Qsv,
    Cuda,
}

impl Into<mirror::VideoDecoderType> for VideoDecoderType {
    fn into(self) -> mirror::VideoDecoderType {
        match self {
            Self::D3D11 => mirror::VideoDecoderType::D3D11,
            Self::Cuda => mirror::VideoDecoderType::Cuda,
            Self::Qsv => mirror::VideoDecoderType::Qsv,
        }
    }
}

#[napi]
#[derive(Debug, Clone, Copy)]
pub enum VideoEncoderType {
    X264,
    Qsv,
    Cuda,
}

impl Into<mirror::VideoEncoderType> for VideoEncoderType {
    fn into(self) -> mirror::VideoEncoderType {
        match self {
            Self::X264 => mirror::VideoEncoderType::X264,
            Self::Cuda => mirror::VideoEncoderType::Cuda,
            Self::Qsv => mirror::VideoEncoderType::Qsv,
        }
    }
}

#[napi(object)]
#[derive(Debug, Clone, Copy)]
pub struct VideoDescriptor {
    pub codec: VideoEncoderType,
    pub frame_rate: u8,
    pub width: u32,
    pub height: u32,
    pub bit_rate: f64,
    pub key_frame_interval: u32,
}

impl Into<mirror::VideoDescriptor> for VideoDescriptor {
    fn into(self) -> mirror::VideoDescriptor {
        mirror::VideoDescriptor {
            codec: self.codec.into(),
            frame_rate: self.frame_rate,
            width: self.width,
            height: self.height,
            bit_rate: self.bit_rate as u64,
            key_frame_interval: self.key_frame_interval,
        }
    }
}

#[napi]
#[derive(Debug, Clone, Copy)]
pub enum SourceType {
    Camera,
    Screen,
    Audio,
}

impl Into<mirror::SourceType> for SourceType {
    fn into(self) -> mirror::SourceType {
        match self {
            Self::Camera => mirror::SourceType::Camera,
            Self::Screen => mirror::SourceType::Screen,
            Self::Audio => mirror::SourceType::Audio,
        }
    }
}

impl From<mirror::SourceType> for SourceType {
    fn from(value: mirror::SourceType) -> Self {
        match value {
            mirror::SourceType::Camera => Self::Camera,
            mirror::SourceType::Screen => Self::Screen,
            mirror::SourceType::Audio => Self::Audio,
        }
    }
}

#[napi(object)]
#[derive(Debug, Clone)]
pub struct SourceDescriptor {
    pub id: String,
    pub name: String,
    pub index: f64,
    pub kind: SourceType,
    pub is_default: bool,
}

impl Into<mirror::Source> for SourceDescriptor {
    fn into(self) -> mirror::Source {
        mirror::Source {
            id: self.id,
            name: self.name,
            kind: self.kind.into(),
            is_default: self.is_default,
            index: self.index as usize,
        }
    }
}

#[napi(object)]
#[derive(Debug, Clone, Copy)]
pub struct AudioDescriptor {
    pub sample_rate: f64,
    pub bit_rate: f64,
}

impl Into<mirror::AudioDescriptor> for AudioDescriptor {
    fn into(self) -> mirror::AudioDescriptor {
        mirror::AudioDescriptor {
            sample_rate: self.sample_rate as u64,
            bit_rate: self.bit_rate as u64,
        }
    }
}

#[napi(object)]
#[derive(Debug, Clone)]
pub struct MirrorSenderVideoDescriptor {
    pub source: SourceDescriptor,
    pub settings: VideoDescriptor,
}

#[napi(object)]
#[derive(Debug, Clone)]
pub struct MirrorSenderAudioDescriptor {
    pub source: SourceDescriptor,
    pub settings: AudioDescriptor,
}

#[napi(object)]
#[derive(Debug, Clone)]
pub struct MirrorSenderServiceDescriptor {
    pub video: Option<MirrorSenderVideoDescriptor>,
    pub audio: Option<MirrorSenderAudioDescriptor>,
    pub multicast: bool,
}

impl Into<MirrorSenderDescriptor> for MirrorSenderServiceDescriptor {
    fn into(self) -> MirrorSenderDescriptor {
        MirrorSenderDescriptor {
            video: self.video.map(|it| (it.source.into(), it.settings.into())),
            audio: self.audio.map(|it| (it.source.into(), it.settings.into())),
            multicast: self.multicast,
        }
    }
}

#[napi(object)]
#[derive(Debug, Clone, Copy)]
pub struct MirrorReceiverServiceDescriptor {
    pub video: VideoDecoderType,
    pub backend: Backend,
}

impl Into<MirrorReceiverDescriptor> for MirrorReceiverServiceDescriptor {
    fn into(self) -> MirrorReceiverDescriptor {
        MirrorReceiverDescriptor {
            video: self.video.into(),
        }
    }
}

#[napi]
pub struct MirrorService(Option<Mirror>);

#[napi]
impl MirrorService {
    #[napi]
    pub fn get_sources(kind: SourceType) -> Vec<SourceDescriptor> {
        Capture::get_sources(kind.into())
            .unwrap_or_else(|_| Vec::new())
            .into_iter()
            .map(|source| SourceDescriptor {
                id: source.id,
                name: source.name,
                index: source.index as f64,
                kind: SourceType::from(source.kind),
                is_default: source.is_default,
            })
            .collect()
    }

    #[napi(constructor)]
    pub fn new(options: MirrorServiceDescriptor) -> napi::Result<Self> {
        let func = || Ok::<_, anyhow::Error>(Self(Some(Mirror::new(options.try_into()?)?)));

        func().map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi(
        ts_args_type = "id: number, options: MirrorSenderServiceDescriptor, callback: () => void"
    )]
    pub fn create_sender(
        &self,
        id: u32,
        options: MirrorSenderServiceDescriptor,
        callback: Function,
    ) -> napi::Result<MirrorSenderService> {
        let func = || {
            Ok::<_, anyhow::Error>(MirrorSenderService(Some(
                self.0
                    .as_ref()
                    .ok_or_else(|| napi::Error::from_reason("mirror is destroy"))?
                    .create_sender(
                        id,
                        options.into(),
                        SilenceSinker(
                            callback
                                .build_threadsafe_function::<()>()
                                .build_callback(|_| Ok(()))?,
                        ),
                    )?,
            )))
        };

        func().map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi(
        ts_args_type = "id: number, options: MirrorReceiverServiceDescriptor, callback: () => void"
    )]
    pub fn create_receiver(
        &self,
        id: u32,
        options: MirrorReceiverServiceDescriptor,
        callback: Function,
    ) -> napi::Result<MirrorReceiverService> {
        let func = || {
            let backend = options.backend;
            Ok::<_, anyhow::Error>(MirrorReceiverService(Some(
                self.0
                    .as_ref()
                    .ok_or_else(|| napi::Error::from_reason("mirror is destroy"))?
                    .create_receiver(
                        id,
                        options.into(),
                        FullDisplaySinker::new(
                            backend,
                            callback
                                .build_threadsafe_function::<()>()
                                .build_callback(|_| Ok(()))?,
                        )?,
                    )?,
            )))
        };

        func().map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn destroy(&mut self) {
        drop(self.0.take());
    }
}

struct SilenceSinker(ThreadsafeFunction<(), JsUnknown, (), false>);

impl FrameSinker for SilenceSinker {
    fn close(&self) {
        self.0.call((), ThreadsafeFunctionCallMode::NonBlocking);
    }
}

#[napi]
pub struct MirrorSenderService(Option<MirrorSender>);

#[napi]
impl MirrorSenderService {
    #[napi(getter, js_name = "multicast")]
    pub fn get_multicast(&self) -> napi::Result<bool> {
        Ok(self
            .0
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("sender is destroy"))?
            .get_multicast())
    }

    #[napi(setter, js_name = "multicast")]
    pub fn set_multicast(&self, value: bool) -> napi::Result<()> {
        self.0
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("sender is destroy"))?
            .set_multicast(value);

        Ok(())
    }

    #[napi]
    pub fn destroy(&mut self) {
        drop(self.0.take());
    }
}

#[napi]
pub struct MirrorReceiverService(Option<MirrorReceiver>);

#[napi]
impl MirrorReceiverService {
    #[napi]
    pub fn destroy(&mut self) {
        drop(self.0.take());
    }
}

enum UserEvent {
    CloseRequested,
    Show,
}

struct Events(EventLoop<UserEvent>);

unsafe impl Send for Events {}
unsafe impl Sync for Events {}

impl Events {
    fn create_proxy(&self) -> EventLoopProxy<UserEvent> {
        self.0.create_proxy()
    }

    fn run(self, app: &mut Views) -> anyhow::Result<()> {
        self.0.run_app(app)?;
        Ok(())
    }
}

struct Views {
    callback: Box<dyn Fn(Result<Arc<Window>, anyhow::Error>)>,
    window: Option<Arc<Window>>,
}

impl ApplicationHandler<UserEvent> for Views {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let mut func = || {
            let mut attr = Window::default_attributes();
            attr.fullscreen = Some(Fullscreen::Borderless(None));
            attr.visible = false;
            attr.resizable = false;
            attr.maximized = false;
            attr.decorations = false;

            let window = Arc::new(event_loop.create_window(attr)?);
            if let Some(monitor) = window.current_monitor() {
                window.set_min_inner_size(Some(monitor.size()));
            }

            window.set_cursor_hittest(false)?;
            self.window = Some(window.clone());

            Ok::<_, anyhow::Error>(window)
        };

        (self.callback)(func());
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            _ => (),
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::CloseRequested => {
                event_loop.exit();
            }
            UserEvent::Show => {
                if let Some(window) = self.window.as_ref() {
                    window.set_visible(true);
                }
            }
        }
    }
}

struct FullDisplaySinker {
    callback: ThreadsafeFunction<(), JsUnknown, (), false>,
    event_loop_proxy: EventLoopProxy<UserEvent>,
    initialized: AtomicBool,
    render: Render,
}

impl FrameSinker for FullDisplaySinker {
    fn video(&self, frame: &VideoFrame) -> bool {
        if !self.initialized.get() {
            self.initialized.update(true);

            if let Err(_) = self.event_loop_proxy.send_event(UserEvent::Show) {
                log::warn!("winit event loop is closed");
            }
        }

        if let Err(e) = self.render.on_video(frame) {
            log::error!("{:?}", e);

            return false;
        }

        true
    }

    fn audio(&self, frame: &AudioFrame) -> bool {
        if let Err(e) = self.render.on_audio(frame) {
            log::error!("{:?}", e);

            return false;
        }

        true
    }

    fn close(&self) {
        if let Err(_) = self.event_loop_proxy.send_event(UserEvent::CloseRequested) {
            log::warn!("winit event loop is closed");
        }

        self.callback
            .call((), ThreadsafeFunctionCallMode::NonBlocking);
    }
}

impl FullDisplaySinker {
    fn new(
        backend: Backend,
        callback: ThreadsafeFunction<(), JsUnknown, (), false>,
    ) -> anyhow::Result<Self> {
        let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
        event_loop.set_control_flow(ControlFlow::Wait);

        let event_loop = Events(event_loop);
        let event_loop_proxy = event_loop.create_proxy();

        let (tx, rx) = channel();
        thread::Builder::new()
            .name("FullDisplayWindowThread".to_string())
            .spawn(move || {
                event_loop
                    .run(&mut Views {
                        window: None,
                        callback: Box::new(move |window| {
                            if let Err(e) = tx.send(window) {
                                log::warn!("{:?}", e);
                            }
                        }),
                    })
                    .unwrap();
            })?;

        let window = rx.recv()??;
        let render = Render::new(
            backend.into(),
            match window.window_handle()?.as_raw() {
                RawWindowHandle::Win32(handle) => mirror::Window(handle.hwnd.get() as *const _),
                _ => unimplemented!("not supports the window handle"),
            },
        )?;

        Ok(Self {
            initialized: AtomicBool::new(false),
            event_loop_proxy,
            callback,
            render,
        })
    }
}
