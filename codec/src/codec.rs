use common::c_str;
use ffmpeg_sys_next::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CodecError {
    #[error("unsupported codecs")]
    NotSupportCodec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoDecoderType {
    H264,
    D3D11,
    Qsv,
    Cuda,
    VideoToolBox,
}

impl ToString for VideoDecoderType {
    fn to_string(&self) -> String {
        match self {
            Self::H264 => "h264",
            Self::D3D11 => "d3d11va",
            Self::Qsv => "h264_qsv",
            Self::Cuda => "h264_cuvid",
            Self::VideoToolBox => "h264_videotoolbox",
        }
        .to_string()
    }
}

impl TryFrom<&str> for VideoDecoderType {
    type Error = CodecError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value {
            "h264" => Self::H264,
            "d3d11va" => Self::D3D11,
            "h264_qsv" => Self::Qsv,
            "h264_cuvid" => Self::Cuda,
            "h264_videotoolbox" => Self::VideoToolBox,
            _ => return Err(CodecError::NotSupportCodec),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoEncoderType {
    X264,
    Qsv,
    Cuda,
    VideoToolBox,
}

impl ToString for VideoEncoderType {
    fn to_string(&self) -> String {
        match self {
            Self::X264 => "libx264",
            Self::Qsv => "h264_qsv",
            Self::Cuda => "h264_nvenc",
            Self::VideoToolBox => "h264_videotoolbox",
        }
        .to_string()
    }
}

impl TryFrom<&str> for VideoEncoderType {
    type Error = CodecError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value {
            "libx264" => Self::X264,
            "h264_qsv" => Self::Qsv,
            "h264_cuvid" => Self::Cuda,
            "h264_videotoolbox" => Self::VideoToolBox,
            _ => return Err(CodecError::NotSupportCodec),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecType {
    Encoder(VideoEncoderType),
    Decoder(VideoDecoderType),
}

impl From<VideoEncoderType> for CodecType {
    fn from(value: VideoEncoderType) -> Self {
        Self::Encoder(value)
    }
}

impl From<VideoDecoderType> for CodecType {
    fn from(value: VideoDecoderType) -> Self {
        Self::Decoder(value)
    }
}

impl CodecType {
    pub fn is_supported(&self) -> bool {
        match self {
            CodecType::Encoder(kind) => {
                if cfg!(target_os = "windows") {
                    *kind != VideoEncoderType::VideoToolBox
                } else if cfg!(target_os = "linux") {
                    *kind == VideoEncoderType::X264
                } else {
                    *kind == VideoEncoderType::VideoToolBox
                }
            }
            CodecType::Decoder(kind) => {
                if cfg!(target_os = "windows") {
                    *kind != VideoDecoderType::VideoToolBox
                } else if cfg!(target_os = "linux") {
                    *kind == VideoDecoderType::H264
                } else {
                    *kind == VideoDecoderType::VideoToolBox
                }
            }
        }
    }

    pub const fn is_encoder(&self) -> bool {
        if let Self::Encoder(_) = self {
            true
        } else {
            false
        }
    }

    pub fn is_qsv(self) -> bool {
        match self {
            CodecType::Encoder(kind) => kind == VideoEncoderType::Qsv,
            CodecType::Decoder(kind) => kind == VideoDecoderType::Qsv,
        }
    }

    pub fn is_hardware(&self) -> bool {
        match self {
            Self::Decoder(codec) => *codec != VideoDecoderType::H264,
            Self::Encoder(codec) => *codec != VideoEncoderType::X264,
        }
    }

    pub unsafe fn find_av_codec(&self) -> *const AVCodec {
        match self {
            Self::Encoder(kind) => avcodec_find_encoder_by_name(c_str!(kind.to_string())),
            Self::Decoder(kind) => {
                if *kind == VideoDecoderType::D3D11 {
                    avcodec_find_decoder(AVCodecID::AV_CODEC_ID_H264)
                } else {
                    avcodec_find_decoder_by_name(c_str!(kind.to_string()))
                }
            }
        }
    }
}
