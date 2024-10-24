mod window;

use self::window::{
    EmptyWindow, LinuxNativeWindowHandle, MacosNativeWindowHandle, NativeWindowHandle, Window,
    WindowsNativeWindowHandle,
};

use std::sync::Arc;

use mirror::{
    AudioDescriptor, Capture, GraphicsBackend, Mirror, Receiver, ReceiverDescriptor, Renderer,
    Sender, SenderDescriptor, Source, SourceType, TransportDescriptor, VideoDecoderType,
    VideoDescriptor, VideoEncoderType,
};

use napi::bindgen_prelude::Function;
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

        mirror::startup()?;
        Ok::<_, anyhow::Error>(())
    };

    func().map_err(|e| napi::Error::from_reason(e.to_string()))
}

/// Roll out the sdk environment and clean up resources.
#[napi]
pub fn shutdown() -> napi::Result<()> {
    mirror::shutdown().map_err(|e| napi::Error::from_reason(e.to_string()))?;

    Ok(())
}

#[napi]
#[derive(Debug, Clone, Copy)]
pub enum MirrorBackend {
    /// Use Direct3D 11.x as a rendering backend, this is not a cross-platform
    /// option and is only available on windows, on some Direct3D 11 only
    /// devices.
    Direct3D11,
    /// This is a new cross-platform backend, and on windows the latency may be
    /// a bit higher than the Direct3D 11 backend.
    WebGPU,
}

impl Into<GraphicsBackend> for MirrorBackend {
    fn into(self) -> GraphicsBackend {
        match self {
            Self::Direct3D11 => GraphicsBackend::Direct3D11,
            Self::WebGPU => GraphicsBackend::WebGPU,
        }
    }
}

/// There's a BrowserWindow API for this:
///
/// ```
/// win.getNativeWindowHandle()
/// ```
///
/// which return the HWND you can use in any native windows code.
#[napi(object)]
#[derive(Clone)]
pub struct MirrorNativeWindowHandle {
    pub windows: Option<WindowsNativeWindowHandle>,
    pub linux: Option<LinuxNativeWindowHandle>,
    pub macos: Option<MacosNativeWindowHandle>,
}

impl Into<NativeWindowHandle> for MirrorNativeWindowHandle {
    fn into(self) -> NativeWindowHandle {
        if let Some(handle) = self.windows {
            return NativeWindowHandle::Windows(handle);
        }

        if let Some(handle) = self.linux {
            return NativeWindowHandle::Linux(handle);
        }

        if let Some(handle) = self.macos {
            return NativeWindowHandle::Macos(handle);
        }

        panic!("You didn't pass any window handles.")
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
    pub backend: MirrorBackend,
    pub window_handle: MirrorNativeWindowHandle,
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
pub enum MirrorVideoDecoderType {
    /// h264 (software)
    H264,
    /// d3d11va
    D3D11,
    /// h264_qsv
    Qsv,
    /// h264_cvuid
    Cuda,
    /// video tool box
    VideoToolBox,
}

impl Into<VideoDecoderType> for MirrorVideoDecoderType {
    fn into(self) -> VideoDecoderType {
        match self {
            Self::H264 => VideoDecoderType::H264,
            Self::D3D11 => VideoDecoderType::D3D11,
            Self::Cuda => VideoDecoderType::Cuda,
            Self::Qsv => VideoDecoderType::Qsv,
            Self::VideoToolBox => VideoDecoderType::VideoToolBox,
        }
    }
}

#[napi]
#[derive(Debug, Clone, Copy)]
pub enum MirrorVideoEncoderType {
    /// libx264 (software)
    X264,
    /// h264_qsv
    Qsv,
    /// h264_nvenc
    Cuda,
    /// video tool box
    VideoToolBox,
}

impl Into<VideoEncoderType> for MirrorVideoEncoderType {
    fn into(self) -> VideoEncoderType {
        match self {
            Self::X264 => VideoEncoderType::X264,
            Self::Cuda => VideoEncoderType::Cuda,
            Self::Qsv => VideoEncoderType::Qsv,
            Self::VideoToolBox => VideoEncoderType::VideoToolBox,
        }
    }
}

#[napi(object)]
#[derive(Debug, Clone, Copy)]
pub struct MirrorVideoDescriptor {
    pub codec: MirrorVideoEncoderType,
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

impl Into<VideoDescriptor> for MirrorVideoDescriptor {
    fn into(self) -> VideoDescriptor {
        VideoDescriptor {
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
pub enum MirrorSourceType {
    /// Camera or video capture card and other devices (and support virtual
    /// camera)
    Camera,
    /// The desktop or monitor corresponds to the desktop in the operating
    /// system.
    Screen,
    /// Audio input and output devices.
    Audio,
}

impl Into<SourceType> for MirrorSourceType {
    fn into(self) -> SourceType {
        match self {
            Self::Camera => SourceType::Camera,
            Self::Screen => SourceType::Screen,
            Self::Audio => SourceType::Audio,
        }
    }
}

impl From<SourceType> for MirrorSourceType {
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
pub struct MirrorSourceDescriptor {
    /// Device ID, usually the symbolic link to the device or the address of the
    /// device file handle.
    pub id: String,
    pub name: String,
    /// Sequence number, which can normally be ignored, in most cases this field
    /// has no real meaning and simply indicates the order in which the device
    /// was acquired internally.
    pub index: f64,
    pub kind: MirrorSourceType,
    /// Whether or not it is the default device, normally used to indicate
    /// whether or not it is the master device.
    pub is_default: bool,
}

impl Into<Source> for MirrorSourceDescriptor {
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

#[napi(object)]
#[derive(Debug, Clone, Copy)]
pub struct MirrorAudioDescriptor {
    pub sample_rate: f64,
    pub bit_rate: f64,
}

impl Into<AudioDescriptor> for MirrorAudioDescriptor {
    fn into(self) -> AudioDescriptor {
        AudioDescriptor {
            sample_rate: self.sample_rate as u64,
            bit_rate: self.bit_rate as u64,
        }
    }
}

#[napi(object)]
#[derive(Debug, Clone)]
pub struct MirrorSenderVideoDescriptor {
    pub source: MirrorSourceDescriptor,
    pub settings: MirrorVideoDescriptor,
}

#[napi(object)]
#[derive(Debug, Clone)]
pub struct MirrorSenderAudioDescriptor {
    pub source: MirrorSourceDescriptor,
    pub settings: MirrorAudioDescriptor,
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
    pub video: MirrorVideoDecoderType,
}

impl Into<ReceiverDescriptor> for MirrorReceiverServiceDescriptor {
    fn into(self) -> ReceiverDescriptor {
        ReceiverDescriptor {
            video: self.video.into(),
        }
    }
}

#[napi]
pub struct MirrorService {
    mirror: Option<Mirror>,
    renderer: Arc<Renderer<'static>>,
}

#[napi]
impl MirrorService {
    #[napi]
    pub fn get_sources(kind: MirrorSourceType) -> Vec<MirrorSourceDescriptor> {
        Capture::get_sources(kind.into())
            .unwrap_or_else(|_| Vec::new())
            .into_iter()
            .map(|source| MirrorSourceDescriptor {
                id: source.id,
                name: source.name,
                index: source.index as f64,
                kind: MirrorSourceType::from(source.kind),
                is_default: source.is_default,
            })
            .collect()
    }

    #[napi(constructor)]
    pub fn new(options: MirrorServiceDescriptor) -> napi::Result<Self> {
        let func = || {
            let window: NativeWindowHandle = options.window_handle.clone().into();
            let size = window.size();

            Ok::<_, anyhow::Error>(Self {
                renderer: Arc::new(Renderer::new(options.backend.into(), window, size)?),
                mirror: Some(Mirror::new(options.try_into()?)?),
            })
        };

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
                self.mirror
                    .as_ref()
                    .ok_or_else(|| napi::Error::from_reason("mirror is destroy"))?
                    .create_sender(
                        id,
                        options.into(),
                        EmptyWindow(
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
            Ok::<_, anyhow::Error>(MirrorReceiverService(Some(
                self.mirror
                    .as_ref()
                    .ok_or_else(|| napi::Error::from_reason("mirror is destroy"))?
                    .create_receiver(
                        id,
                        options.into(),
                        Window {
                            renderer: self.renderer.clone(),
                            callback: callback
                                .build_threadsafe_function::<()>()
                                .build_callback(|_| Ok(()))?,
                        },
                    )?,
            )))
        };

        func().map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn destroy(&mut self) {
        drop(self.mirror.take());
    }
}

#[napi]
pub struct MirrorSenderService(Option<Sender<EmptyWindow>>);

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
pub struct MirrorReceiverService(Option<Receiver<Window>>);

#[napi]
impl MirrorReceiverService {
    #[napi]
    pub fn destroy(&mut self) {
        drop(self.0.take());
    }
}
