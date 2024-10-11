use parking_lot::{Mutex, RwLock};
use std::{
    sync::{atomic::AtomicBool, Arc},
    thread,
};

use anyhow::anyhow;
use crossbeam_utils::sync::Parker;
use mirror::{
    AVFrameSink, AVFrameStream, AudioFrame, Capture, Close, GraphicsBackend, Mirror, Receiver,
    ReceiverDescriptor, Renderer, Sender, SenderDescriptor, TransportDescriptor, VideoFrame,
};

use napi::{
    bindgen_prelude::Function,
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
    JsUnknown,
};

use common::{atomic::EasyAtomic, logger, Size};
use napi_derive::napi;
use once_cell::sync::Lazy;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    platform::run_on_demand::EventLoopExtRunOnDemand,
    window::{Fullscreen, Window, WindowId},
};

static WINDOW: Lazy<RwLock<Option<Arc<Window>>>> = Lazy::new(|| RwLock::new(None));
static EVENT_LOOP: Lazy<RwLock<Option<EventLoopProxy<AppEvent>>>> = Lazy::new(|| RwLock::new(None));

enum AppEvent {
    CloseRequested,
    Show,
    Hide,
}

struct App {
    window: Option<Arc<Window>>,
    callback: Option<Box<dyn FnOnce(anyhow::Result<()>)>>,
}

impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let callback = self.callback.take().unwrap();

        // The default window created is invisible, and the window is made visible by
        // receiving external requests.
        let mut attr = Window::default_attributes();
        attr.visible = false;
        attr.resizable = false;
        attr.maximized = false;
        attr.decorations = false;
        attr.fullscreen = Some(Fullscreen::Borderless(None));

        callback((|| {
            let window = Arc::new(event_loop.create_window(attr)?);
            let monitor = window
                .current_monitor()
                .ok_or_else(|| anyhow!("not found a monitor"))?;

            window.set_min_inner_size(Some(monitor.size()));
            window.set_cursor_hittest(false)?;

            self.window = Some(window.clone());
            WINDOW.write().replace(window);
            Ok::<_, anyhow::Error>(())
        })())
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

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::CloseRequested => {
                event_loop.exit();
            }
            AppEvent::Show => {
                if let Some(window) = self.window.as_ref() {
                    window.set_visible(true);
                }
            }
            AppEvent::Hide => {
                if let Some(window) = self.window.as_ref() {
                    window.set_visible(false);
                }
            }
        }
    }
}

struct Events(EventLoop<AppEvent>);

unsafe impl Sync for Events {}
unsafe impl Send for Events {}

impl Events {
    fn run(&mut self, mut app: App) {
        self.0.run_app_on_demand(&mut app).unwrap();
    }
}

/// To initialize the environment.
#[napi]
#[allow(unused_variables)]
pub fn startup(user_data: Option<String>) -> napi::Result<()> {
    let func = || {
        logger::init_logger(
            log::LevelFilter::Info,
            user_data.as_ref().map(|x| x.as_str()),
        )?;

        std::panic::set_hook(Box::new(|info| {
            log::error!(
                "pnaic: location={:?}, message={:?}",
                info.location(),
                info.payload().downcast_ref::<String>(),
            );
        }));

        {
            let event_loop = EventLoop::<AppEvent>::with_user_event().build()?;
            event_loop.set_control_flow(ControlFlow::Wait);

            let event_loop_proxy = event_loop.create_proxy();
            EVENT_LOOP.write().replace(event_loop_proxy);

            let parker = Parker::new();
            let unparker = parker.unparker().clone();

            let result = Arc::new(Mutex::new(Ok(())));
            let result_ = result.clone();

            let mut events = Events(event_loop);
            thread::spawn(move || {
                events.run(App {
                    window: None,
                    callback: Some(Box::new(move |result| {
                        *result_.lock() = result;
                        unparker.unpark();
                    })),
                });
            });

            parker.park();
            result
                .lock()
                .as_ref()
                .cloned()
                .map_err(|e| anyhow!("{:?}", e))?;
        }

        mirror::startup()?;
        Ok::<_, anyhow::Error>(())
    };

    func().map_err(|e| napi::Error::from_reason(e.to_string()))
}

/// Roll out the sdk environment and clean up resources.
#[napi]
pub fn shutdown() -> napi::Result<()> {
    mirror::shutdown().map_err(|e| napi::Error::from_reason(e.to_string()))?;

    if let Some(event_loop) = EVENT_LOOP.read().as_ref() {
        if let Err(_) = event_loop.send_event(AppEvent::CloseRequested) {
            log::warn!("winit event loop is closed");
        }
    }

    Ok(())
}

#[napi]
#[derive(Debug, Clone, Copy)]
pub enum Backend {
    /// Use Direct3D 11.x as a rendering backend, this is not a cross-platform
    /// option and is only available on windows, on some Direct3D 11 only
    /// devices.
    Direct3D11,
    /// This is a new cross-platform backend, and on windows the latency may be
    /// a bit higher than the Direct3D 11 backend.
    WebGPU,
}

impl Into<GraphicsBackend> for Backend {
    fn into(self) -> GraphicsBackend {
        match self {
            Self::Direct3D11 => GraphicsBackend::Direct3D11,
            Self::WebGPU => GraphicsBackend::WebGPU,
        }
    }
}

#[napi(object)]
pub struct MirrorServiceDescriptor {
    /// The IP address and port of the server, in this case the service refers
    /// to the mirror service.
    pub server: String,
    /// The multicast address used for multicasting, which is an IP address.
    pub multicast: String,
    /// see: https://en.wikipedia.org/wiki/Maximum_transmission_unit
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
    /// d3d11va
    D3D11,
    /// h264_qsv
    Qsv,
    /// h264_cvuid
    Cuda,
    /// h264 videotoolbox
    VideoToolBox,
}

impl Into<mirror::VideoDecoderType> for VideoDecoderType {
    fn into(self) -> mirror::VideoDecoderType {
        match self {
            Self::D3D11 => mirror::VideoDecoderType::D3D11,
            Self::Cuda => mirror::VideoDecoderType::Cuda,
            Self::Qsv => mirror::VideoDecoderType::Qsv,
            Self::VideoToolBox => mirror::VideoDecoderType::VideoToolBox,
        }
    }
}

#[napi]
#[derive(Debug, Clone, Copy)]
pub enum VideoEncoderType {
    /// libx264
    X264,
    /// h264_qsv
    Qsv,
    /// h264_nvenc
    Cuda,
    /// h264 videotoolbox
    VideoToolBox,
}

impl Into<mirror::VideoEncoderType> for VideoEncoderType {
    fn into(self) -> mirror::VideoEncoderType {
        match self {
            Self::X264 => mirror::VideoEncoderType::X264,
            Self::Cuda => mirror::VideoEncoderType::Cuda,
            Self::Qsv => mirror::VideoEncoderType::Qsv,
            Self::VideoToolBox => mirror::VideoEncoderType::VideoToolBox,
        }
    }
}

#[napi(object)]
#[derive(Debug, Clone, Copy)]
pub struct VideoDescriptor {
    pub codec: VideoEncoderType,
    ///  For codecs that store a framerate value in the compressed bitstream,
    /// the decoder may export it here.
    pub frame_rate: u8,
    /// picture width / height.
    pub width: u32,
    /// picture width / height.
    pub height: u32,
    pub bit_rate: f64,
    /// the number of pictures in a group of pictures, or 0 for intra_only.
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
    /// Whether to use multicast.
    pub multicast: bool,
}

impl Into<SenderDescriptor> for MirrorSenderServiceDescriptor {
    fn into(self) -> SenderDescriptor {
        SenderDescriptor {
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

impl Into<ReceiverDescriptor> for MirrorReceiverServiceDescriptor {
    fn into(self) -> ReceiverDescriptor {
        ReceiverDescriptor {
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
                        Viewer::new(
                            Backend::WebGPU,
                            callback
                                .build_threadsafe_function::<()>()
                                .build_callback(|_| Ok(()))?,
                        )?,
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
                        Viewer::new(
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

#[napi]
pub struct MirrorSenderService(Option<Sender>);

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
pub struct MirrorReceiverService(Option<Receiver>);

#[napi]
impl MirrorReceiverService {
    #[napi]
    pub fn destroy(&mut self) {
        drop(self.0.take());
    }
}

struct Viewer {
    callback: ThreadsafeFunction<(), JsUnknown, (), false>,
    initialized: AtomicBool,
    render: Renderer<'static>,
}

impl AVFrameStream for Viewer {}

impl AVFrameSink for Viewer {
    fn video(&self, frame: &VideoFrame) -> bool {
        if !self.initialized.get() {
            self.initialized.update(true);

            if let Some(event_loop) = EVENT_LOOP.read().as_ref() {
                if let Err(_) = event_loop.send_event(AppEvent::Show) {
                    log::warn!("winit event loop is closed");
                }
            }
        }

        self.render.video(frame)
    }

    fn audio(&self, frame: &AudioFrame) -> bool {
        self.render.audio(frame)
    }
}

impl Close for Viewer {
    fn close(&self) {
        if let Some(event_loop) = EVENT_LOOP.read().as_ref() {
            if let Err(_) = event_loop.send_event(AppEvent::Hide) {
                log::warn!("winit event loop is closed");
            }
        }

        self.callback
            .call((), ThreadsafeFunctionCallMode::NonBlocking);
    }
}

impl Viewer {
    fn new(
        backend: Backend,
        callback: ThreadsafeFunction<(), JsUnknown, (), false>,
    ) -> anyhow::Result<Self> {
        let window = WINDOW.read().as_ref().cloned().unwrap();
        let inner_size = window.inner_size();
        let render = Renderer::new(
            backend.into(),
            window,
            Size {
                width: inner_size.width,
                height: inner_size.height,
            },
        )?;

        Ok(Self {
            initialized: AtomicBool::new(false),
            callback,
            render,
        })
    }
}
