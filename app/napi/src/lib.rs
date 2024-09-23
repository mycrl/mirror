use std::{
    sync::{mpsc::channel, Arc},
    thread,
};

use mirror::{
    AudioFrame, FrameSinker, Mirror, MirrorReceiver, MirrorReceiverDescriptor, MirrorSender,
    MirrorSenderDescriptor, Render, TransportDescriptor, VideoFrame,
};

use napi::{
    bindgen_prelude::Function,
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
    JsUnknown,
};

use napi_derive::napi;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::{Fullscreen, Window, WindowId},
};

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

#[napi(object)]
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
pub struct MirrorSenderVideoDescriptor {
    pub source: SourceDescriptor,
    pub settings: VideoDescriptor,
}

#[napi(object)]
pub struct MirrorSenderAudioDescriptor {
    pub source: SourceDescriptor,
    pub settings: AudioDescriptor,
}

#[napi(object)]
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
pub struct MirrorReceiverServiceDescriptor {
    pub video: VideoDecoderType,
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
    #[napi(constructor)]
    pub fn new(options: MirrorServiceDescriptor) -> napi::Result<Self> {
        let func = || Ok::<_, anyhow::Error>(Self(Some(Mirror::new(options.try_into()?)?)));

        func().map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
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

    #[napi]
    pub fn create_receiver(
        &self,
        id: u32,
        options: MirrorReceiverServiceDescriptor,
        callback: Function,
    ) -> napi::Result<MirrorReceiverService> {
        let func = || {
            Ok::<_, anyhow::Error>(MirrorReceiverService(Some(
                self.0
                    .as_ref()
                    .ok_or_else(|| napi::Error::from_reason("mirror is destroy"))?
                    .create_receiver(
                        id,
                        options.into(),
                        FullDisplaySinker::new(
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
        let mut attr = Window::default_attributes();
        attr.fullscreen = Some(Fullscreen::Borderless(None));

        let window = Arc::new(event_loop.create_window(attr).unwrap());

        (self.callback)(Ok(window.clone()));
        self.window = Some(window);
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
        }
    }
}

struct FullDisplaySinker {
    callback: ThreadsafeFunction<(), JsUnknown, (), false>,
    event_loop_proxy: EventLoopProxy<UserEvent>,
    render: Render,
}

impl FrameSinker for FullDisplaySinker {
    fn video(&self, frame: &VideoFrame) -> bool {
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
    fn new(callback: ThreadsafeFunction<(), JsUnknown, (), false>) -> anyhow::Result<Self> {
        let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
        event_loop.set_control_flow(ControlFlow::Wait);

        let event_loop = Events(event_loop);
        let event_loop_proxy = event_loop.create_proxy();

        let (tx, rx) = channel();
        thread::Builder::new()
            .name("FullDisplayWindowThread".to_string())
            .spawn(move || {
                event_loop.run(&mut Views {
                    window: None,
                    callback: Box::new(move |window| {
                        if let Err(e) = tx.send(window) {
                            log::warn!("{:?}", e);
                        }
                    }),
                }).unwrap();
            })?;

        let window = rx.recv()??;
        let render = Render::new(match window.window_handle()?.as_raw() {
            RawWindowHandle::Win32(handle) => mirror::Window(handle.hwnd.get() as *const _),
            _ => unimplemented!("not supports the window handle"),
        })?;

        Ok(Self {
            event_loop_proxy,
            callback,
            render,
        })
    }
}
