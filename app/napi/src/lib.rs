use mirror::{
    AudioFrame, FrameSinker, Mirror, Render, SenderDescriptor, TransportDescriptor, VideoFrame,
};

use napi::{
    bindgen_prelude::Function,
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
    JsUnknown,
};

use napi_derive::napi;

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

impl Into<SenderDescriptor> for MirrorSenderServiceDescriptor {
    fn into(self) -> SenderDescriptor {
        SenderDescriptor {
            video: self.video.map(|it| (it.source.into(), it.settings.into())),
            audio: self.audio.map(|it| (it.source.into(), it.settings.into())),
            multicast: self.multicast,
        }
    }
}

#[napi]
pub struct MirrorService {
    mirror: Mirror,
    renderer: Render,
}

#[napi]
impl MirrorService {
    #[napi(constructor)]
    pub fn new(options: MirrorServiceDescriptor) -> napi::Result<Self> {
        let func = || {
            Ok::<_, anyhow::Error>(Self {
                mirror: Mirror::new(options.try_into()?)?,
                renderer: Render::new(window)?,
            })
        };

        func().map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn create_sender(
        &self,
        id: u32,
        options: MirrorSenderServiceDescriptor,
        callback: Function,
    ) {
        let func = || {
            let func = callback
                .build_threadsafe_function::<()>()
                .build_callback(|_| Ok(()))?;

            self.mirror
                .create_sender(id, options.into(), SilenceSinker(func));

            Ok::<_, anyhow::Error>(())
        };

        func().map_err(|e| napi::Error::from_reason(e.to_string()));
    }
}

struct SilenceSinker(ThreadsafeFunction<(), JsUnknown, (), false>);

impl FrameSinker for SilenceSinker {
    fn audio(&self, _frame: &AudioFrame) -> bool {
        true
    }

    fn video(&self, _frame: &VideoFrame) -> bool {
        true
    }

    fn close(&self) {
        self.0.call((), ThreadsafeFunctionCallMode::NonBlocking);
    }
}

struct Viewer {}
