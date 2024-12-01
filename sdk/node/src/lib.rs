use std::net::SocketAddr;

use hylarana::{
    raw_window_handle::{
        DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawWindowHandle,
        Win32WindowHandle, WindowHandle,
    },
    AVFrameObserver, AVFrameStreamPlayer, AVFrameStreamPlayerOptions, AudioOptions, Capture,
    Hylarana, HylaranaReceiver, HylaranaReceiverCodecOptions, HylaranaReceiverOptions,
    HylaranaSender, HylaranaSenderMediaOptions, HylaranaSenderOptions, HylaranaSenderTrackOptions,
    Size, Source, SourceType, TransportOptions, TransportStrategy, VideoDecoderType,
    VideoEncoderType, VideoOptions, VideoRenderBackend, VideoRenderOptions,
};

use napi::{
    bindgen_prelude::Function,
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
    JsBigInt, JsUnknown,
};

use napi_derive::napi;

/// To initialize the environment.
#[napi]
#[allow(unused_variables)]
pub fn startup(user_data: Option<String>) -> napi::Result<()> {
    let func = || {
        simple_logger::init_with_level(log::Level::Info)?;

        std::panic::set_hook(Box::new(|info| {
            log::error!(
                "pnaic: location={:?}, message={:?}",
                info.location(),
                info.payload().downcast_ref::<String>(),
            );
        }));

        hylarana::startup()?;
        Ok::<_, anyhow::Error>(())
    };

    func().map_err(|e| napi::Error::from_reason(e.to_string()))
}

/// Roll out the sdk environment and clean up resources.
#[napi]
pub fn shutdown() -> napi::Result<()> {
    hylarana::shutdown().map_err(|e| napi::Error::from_reason(e.to_string()))?;

    Ok(())
}

#[napi(object)]
#[derive(Clone)]
pub struct HylaranaWindow {
    pub hwnd: JsBigInt,
    pub width: u32,
    pub height: u32,
}

unsafe impl Send for HylaranaWindow {}
unsafe impl Sync for HylaranaWindow {}

impl HylaranaWindow {
    pub fn size(&self) -> Size {
        Size {
            width: self.width,
            height: self.height,
        }
    }
}

impl HasDisplayHandle for HylaranaWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::windows())
    }
}

impl HasWindowHandle for HylaranaWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Ok(unsafe {
            WindowHandle::borrow_raw(RawWindowHandle::Win32(Win32WindowHandle::new(
                std::num::NonZeroIsize::new(self.hwnd.get_i64().unwrap().0 as isize).unwrap(),
            )))
        })
    }
}

#[napi]
#[derive(Debug, Clone, Copy)]
pub enum HylaranaVideoRenderBackend {
    /// Use Direct3D 11.x as a rendering backend, this is not a cross-platform
    /// option and is only available on windows, on some Direct3D 11 only
    /// devices.
    Direct3D11,
    /// This is a new cross-platform backend, and on windows the latency may be
    /// a bit higher than the Direct3D 11 backend.
    WebGPU,
}

impl Into<VideoRenderBackend> for HylaranaVideoRenderBackend {
    fn into(self) -> VideoRenderBackend {
        match self {
            Self::Direct3D11 => VideoRenderBackend::Direct3D11,
            Self::WebGPU => VideoRenderBackend::WebGPU,
        }
    }
}

#[napi]
#[derive(Clone)]
pub enum HylaranaStrategy {
    Direct,
    Relay,
    Multicast,
}

#[napi(object)]
#[derive(Clone)]
pub struct HylaranaTransportOptions {
    pub strategy: HylaranaStrategy,
    pub address: String,
    pub mtu: u32,
}

impl TryInto<TransportOptions> for HylaranaTransportOptions {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<TransportOptions, Self::Error> {
        let address: SocketAddr = self.address.parse()?;

        Ok(TransportOptions {
            mtu: self.mtu as usize,
            strategy: match self.strategy {
                HylaranaStrategy::Relay => TransportStrategy::Relay(address),
                HylaranaStrategy::Direct => TransportStrategy::Direct(address),
                HylaranaStrategy::Multicast => TransportStrategy::Multicast(address),
            },
        })
    }
}

#[napi(object)]
#[derive(Clone)]
pub struct HylaranaVideoPlayerOptions {
    pub window: HylaranaWindow,
    pub backend: HylaranaVideoRenderBackend,
}

impl Into<VideoRenderOptions<HylaranaWindow>> for HylaranaVideoPlayerOptions {
    fn into(self) -> VideoRenderOptions<HylaranaWindow> {
        VideoRenderOptions {
            backend: self.backend.into(),
            size: self.window.size(),
            target: self.window,
        }
    }
}

#[napi]
#[derive(Debug, Clone, Copy)]
pub enum HylaranaVideoDecoderType {
    /// h264 (software)
    H264,
    /// d3d11va
    D3D11,
    /// h264_qsv
    Qsv,
    /// video tool box
    VideoToolBox,
}

impl Into<VideoDecoderType> for HylaranaVideoDecoderType {
    fn into(self) -> VideoDecoderType {
        match self {
            Self::H264 => VideoDecoderType::H264,
            Self::D3D11 => VideoDecoderType::D3D11,
            Self::Qsv => VideoDecoderType::Qsv,
            Self::VideoToolBox => VideoDecoderType::VideoToolBox,
        }
    }
}

#[napi]
#[derive(Debug, Clone, Copy)]
pub enum HylaranaVideoEncoderType {
    /// libx264 (software)
    X264,
    /// h264_qsv
    Qsv,
    /// video tool box
    VideoToolBox,
}

impl Into<VideoEncoderType> for HylaranaVideoEncoderType {
    fn into(self) -> VideoEncoderType {
        match self {
            Self::X264 => VideoEncoderType::X264,
            Self::Qsv => VideoEncoderType::Qsv,
            Self::VideoToolBox => VideoEncoderType::VideoToolBox,
        }
    }
}

#[napi(object)]
#[derive(Debug, Clone, Copy)]
pub struct HylaranaVideoOptions {
    pub codec: HylaranaVideoEncoderType,
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

impl Into<VideoOptions> for HylaranaVideoOptions {
    fn into(self) -> VideoOptions {
        VideoOptions {
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
pub enum HylaranaSourceType {
    /// Camera or video capture card and other devices (and support virtual
    /// camera)
    Camera,
    /// The desktop or monitor corresponds to the desktop in the operating
    /// system.
    Screen,
    /// Audio input and output devices.
    Audio,
}

impl Into<SourceType> for HylaranaSourceType {
    fn into(self) -> SourceType {
        match self {
            Self::Camera => SourceType::Camera,
            Self::Screen => SourceType::Screen,
            Self::Audio => SourceType::Audio,
        }
    }
}

impl From<SourceType> for HylaranaSourceType {
    fn from(value: SourceType) -> Self {
        match value {
            SourceType::Camera => Self::Camera,
            SourceType::Screen => Self::Screen,
            SourceType::Audio => Self::Audio,
        }
    }
}

#[napi(object)]
#[derive(Debug, Clone)]
pub struct HylaranaSourceOptions {
    /// Device ID, usually the symbolic link to the device or the address of the
    /// device file handle.
    pub id: String,
    pub name: String,
    /// Sequence number, which can normally be ignored, in most cases this field
    /// has no real meaning and simply indicates the order in which the device
    /// was acquired internally.
    pub index: f64,
    pub kind: HylaranaSourceType,
    /// Whether or not it is the default device, normally used to indicate
    /// whether or not it is the master device.
    pub is_default: bool,
}

impl Into<Source> for HylaranaSourceOptions {
    fn into(self) -> Source {
        Source {
            id: self.id,
            name: self.name,
            kind: self.kind.into(),
            is_default: self.is_default,
            index: self.index as usize,
        }
    }
}

#[napi]
pub fn get_sources(kind: HylaranaSourceType) -> Vec<HylaranaSourceOptions> {
    Capture::get_sources(kind.into())
        .unwrap_or_else(|_| Vec::new())
        .into_iter()
        .map(|source| HylaranaSourceOptions {
            id: source.id,
            name: source.name,
            index: source.index as f64,
            kind: HylaranaSourceType::from(source.kind),
            is_default: source.is_default,
        })
        .collect()
}

#[napi(object)]
#[derive(Debug, Clone, Copy)]
pub struct HylaranaAudioOptions {
    pub sample_rate: f64,
    pub bit_rate: f64,
}

impl Into<AudioOptions> for HylaranaAudioOptions {
    fn into(self) -> AudioOptions {
        AudioOptions {
            sample_rate: self.sample_rate as u64,
            bit_rate: self.bit_rate as u64,
        }
    }
}

#[napi(object)]
pub struct HylaranaSenderVideoTrackOptions {
    pub source: HylaranaSourceOptions,
    pub options: HylaranaVideoOptions,
}

impl Into<HylaranaSenderTrackOptions<VideoOptions>> for HylaranaSenderVideoTrackOptions {
    fn into(self) -> HylaranaSenderTrackOptions<VideoOptions> {
        HylaranaSenderTrackOptions {
            source: self.source.into(),
            options: self.options.into(),
        }
    }
}

#[napi(object)]
pub struct HylaranaSenderAudioTrackOptions {
    pub source: HylaranaSourceOptions,
    pub options: HylaranaAudioOptions,
}

impl Into<HylaranaSenderTrackOptions<AudioOptions>> for HylaranaSenderAudioTrackOptions {
    fn into(self) -> HylaranaSenderTrackOptions<AudioOptions> {
        HylaranaSenderTrackOptions {
            source: self.source.into(),
            options: self.options.into(),
        }
    }
}

#[napi(object)]
pub struct HylaranaSenderMediaTrackOptions {
    pub video: Option<HylaranaSenderVideoTrackOptions>,
    pub audio: Option<HylaranaSenderAudioTrackOptions>,
}

impl Into<HylaranaSenderMediaOptions> for HylaranaSenderMediaTrackOptions {
    fn into(self) -> HylaranaSenderMediaOptions {
        HylaranaSenderMediaOptions {
            video: self.video.map(|it| it.into()),
            audio: self.audio.map(|it| it.into()),
        }
    }
}

/// Configuration of the audio and video streaming player.
#[napi]
#[derive(Clone)]
pub enum HylaranaPlayerOptionsType {
    /// Play video only.
    OnlyVideo,
    /// Both audio and video will play.
    All,
    /// Play audio only.
    OnlyAudio,
    /// Nothing plays.
    Quiet,
}

#[napi(object)]
#[derive(Clone)]
pub struct HylaranaPlayerOptions {
    pub kind: HylaranaPlayerOptionsType,
    pub value: Option<HylaranaVideoPlayerOptions>,
}

impl Into<AVFrameStreamPlayerOptions<HylaranaWindow>> for HylaranaPlayerOptions {
    fn into(self) -> AVFrameStreamPlayerOptions<HylaranaWindow> {
        type Ty = HylaranaPlayerOptionsType;

        match self {
            Self {
                kind: Ty::OnlyVideo,
                value: Some(it),
            } => AVFrameStreamPlayerOptions::OnlyVideo(it.into()),
            Self {
                kind: Ty::All,
                value: Some(it),
            } => AVFrameStreamPlayerOptions::All(it.into()),
            Self {
                kind: Ty::OnlyAudio,
                value: None,
            } => AVFrameStreamPlayerOptions::OnlyAudio,
            Self {
                kind: Ty::Quiet,
                value: None,
            } => AVFrameStreamPlayerOptions::Quiet,
            _ => unreachable!(),
        }
    }
}

#[napi(object)]
pub struct HylaranaSenderServiceOptions {
    pub media: HylaranaSenderMediaTrackOptions,
    pub transport: HylaranaTransportOptions,
    pub player: HylaranaPlayerOptions,
}

impl TryInto<HylaranaSenderOptions> for HylaranaSenderServiceOptions {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<HylaranaSenderOptions, Self::Error> {
        Ok(HylaranaSenderOptions {
            media: self.media.into(),
            transport: self.transport.try_into()?,
        })
    }
}

struct Callback(ThreadsafeFunction<(), JsUnknown, (), false>);

impl AVFrameObserver for Callback {
    fn close(&self) {
        self.0.call((), ThreadsafeFunctionCallMode::NonBlocking);
    }
}

#[napi(ts_args_type = "options: HylaranaSenderServiceOptions, callback: () => void")]
pub fn create_sender(
    options: HylaranaSenderServiceOptions,
    callback: Function,
) -> napi::Result<HylaranaSenderService> {
    let func = || {
        let player = AVFrameStreamPlayer::new(
            options.player.clone().into(),
            Callback(
                callback
                    .build_threadsafe_function::<()>()
                    .build_callback(|it| Ok(it.value))?,
            ),
        )?;

        Ok::<_, anyhow::Error>(HylaranaSenderService(Some(Hylarana::create_sender(
            options.try_into()?,
            player,
        )?)))
    };

    func().map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi(object)]
pub struct HylaranaReceiverMediaCodecOptions {
    pub video: HylaranaVideoDecoderType,
}

impl Into<HylaranaReceiverCodecOptions> for HylaranaReceiverMediaCodecOptions {
    fn into(self) -> HylaranaReceiverCodecOptions {
        HylaranaReceiverCodecOptions {
            video: self.video.into(),
        }
    }
}

#[napi(object)]
pub struct HylaranaReceiverServiceOptions {
    pub codec: HylaranaReceiverMediaCodecOptions,
    pub transport: HylaranaTransportOptions,
    pub player: HylaranaPlayerOptions,
}

impl TryInto<HylaranaReceiverOptions> for HylaranaReceiverServiceOptions {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<HylaranaReceiverOptions, Self::Error> {
        Ok(HylaranaReceiverOptions {
            codec: self.codec.into(),
            transport: self.transport.try_into()?,
        })
    }
}

#[napi(ts_args_type = "id: string, options: HylaranaReceiverServiceOptions, callback: () => void")]
pub fn create_receiver(
    id: String,
    options: HylaranaReceiverServiceOptions,
    callback: Function,
) -> napi::Result<HylaranaReceiverService> {
    let func = || {
        let player = AVFrameStreamPlayer::new(
            options.player.clone().into(),
            Callback(
                callback
                    .build_threadsafe_function::<()>()
                    .build_callback(|it| Ok(it.value))?,
            ),
        )?;

        Ok::<_, anyhow::Error>(HylaranaReceiverService(Some(Hylarana::create_receiver(
            id,
            options.try_into()?,
            player,
        )?)))
    };

    func().map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
pub struct HylaranaSenderService(Option<HylaranaSender<AVFrameStreamPlayer<'static, Callback>>>);

#[napi]
impl HylaranaSenderService {
    #[napi]
    pub fn destroy(&mut self) {
        drop(self.0.take());
    }
}

#[napi]
pub struct HylaranaReceiverService(
    Option<HylaranaReceiver<AVFrameStreamPlayer<'static, Callback>>>,
);

#[napi]
impl HylaranaReceiverService {
    #[napi]
    pub fn destroy(&mut self) {
        drop(self.0.take());
    }
}
