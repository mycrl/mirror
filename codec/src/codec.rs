use std::str::FromStr;

use mirror_common::{c_str, ffmpeg::*};
use thiserror::Error;

#[cfg(any(target_os = "windows", target_os = "macos"))]
use mirror_common::Size;

#[cfg(target_os = "windows")]
use mirror_common::win32::{windows::core::Interface, Direct3DDevice};

#[derive(Error, Debug)]
pub enum CreateVideoContextError {
    #[error("not found av codec")]
    NotFoundAVCodec,
    #[error("failed to alloc av context")]
    AllocAVContextError,
    #[error("failed to alloc av hardware device context")]
    AllocAVHardwareDeviceContextError,
    #[error("missing direct3d device")]
    MissingDirect3DDevice,
    #[cfg(target_os = "windows")]
    #[error(transparent)]
    SetMultithreadProtectedError(#[from] mirror_common::win32::windows::core::Error),
    #[error("failed to init av hardware device context")]
    InitAVHardwareDeviceContextError,
    #[error("failed to init qsv device context")]
    InitQsvDeviceContextError,
    #[error("failed to alloc av hardware frame context")]
    AllocAVHardwareFrameContextError,
    #[error("failed to init av hardware frame context")]
    InitAVHardwareFrameContextError,
}

#[derive(Debug, Error, Clone, Copy)]
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

impl FromStr for VideoDecoderType {
    type Err = CodecError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
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

impl FromStr for VideoEncoderType {
    type Err = CodecError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
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
                    *kind == VideoEncoderType::X264 || *kind == VideoEncoderType::VideoToolBox
                }
            }
            CodecType::Decoder(kind) => {
                if cfg!(target_os = "windows") {
                    *kind != VideoDecoderType::VideoToolBox
                } else if cfg!(target_os = "linux") {
                    *kind == VideoDecoderType::H264
                } else {
                    *kind == VideoDecoderType::H264 || *kind == VideoDecoderType::VideoToolBox
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
                if *kind == VideoDecoderType::D3D11 || *kind == VideoDecoderType::VideoToolBox {
                    avcodec_find_decoder(AVCodecID::AV_CODEC_ID_H264)
                } else {
                    avcodec_find_decoder_by_name(c_str!(kind.to_string()))
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub fn create_video_context(
    context: &mut *mut AVCodecContext,
    kind: CodecType,
    size: Option<Size>,
    direct3d: Option<Direct3DDevice>,
) -> Result<*const AVCodec, CreateVideoContextError> {
    // It is not possible to directly find the d3d11va decoder, so special
    // processing is required here. For d3d11va, the hardware context is initialized
    // below.
    let codec = unsafe { kind.find_av_codec() };
    if codec.is_null() {
        return Err(CreateVideoContextError::NotFoundAVCodec);
    }

    *context = unsafe { avcodec_alloc_context3(codec) };
    if context.is_null() {
        return Err(CreateVideoContextError::AllocAVContextError);
    }

    // The hardware codec is used, and the hardware context is initialized here for
    // the hardware codec.
    if kind.is_hardware() {
        let hw_device_ctx =
            unsafe { av_hwdevice_ctx_alloc(AVHWDeviceType::AV_HWDEVICE_TYPE_D3D11VA) };
        if hw_device_ctx.is_null() {
            return Err(CreateVideoContextError::AllocAVHardwareDeviceContextError);
        }

        // Use externally created d3d devices and do not let ffmpeg create d3d devices
        // itself.
        let direct3d = if let Some(direct3d) = direct3d {
            direct3d
        } else {
            return Err(CreateVideoContextError::MissingDirect3DDevice);
        };

        // Special handling is required for qsv, which requires multithreading to be
        // enabled for the d3d device.
        if kind.is_qsv() {
            if let Err(e) = direct3d.set_multithread_protected(true) {
                return Err(CreateVideoContextError::SetMultithreadProtectedError(e));
            }
        }

        let d3d11_hwctx = unsafe {
            let hwctx = (&mut *hw_device_ctx).data as *mut AVHWDeviceContext;
            &mut *((&mut *hwctx).hwctx as *mut AVD3D11VADeviceContext)
        };

        d3d11_hwctx.device = direct3d.device.as_raw() as *mut _;
        d3d11_hwctx.device_context = direct3d.context.as_raw() as *mut _;

        if unsafe { av_hwdevice_ctx_init(hw_device_ctx) } != 0 {
            return Err(CreateVideoContextError::InitAVHardwareDeviceContextError);
        }

        // Creating a qsv device is a little different, the qsv hardware context needs
        // to be derived from the platform's native hardware context.
        let context_mut = unsafe { &mut **context };
        if kind.is_qsv() {
            let mut qsv_device_ctx = std::ptr::null_mut();
            if unsafe {
                av_hwdevice_ctx_create_derived(
                    &mut qsv_device_ctx,
                    AVHWDeviceType::AV_HWDEVICE_TYPE_QSV,
                    hw_device_ctx,
                    0,
                )
            } != 0
            {
                return Err(CreateVideoContextError::InitQsvDeviceContextError);
            }

            unsafe {
                context_mut.hw_device_ctx = av_buffer_ref(qsv_device_ctx);
            }

            // Similarly, the qsv hardware frame also needs to be created and initialized
            // independently.
            if kind.is_encoder() {
                let hw_frames_ctx = unsafe { av_hwframe_ctx_alloc(context_mut.hw_device_ctx) };
                if hw_frames_ctx.is_null() {
                    return Err(CreateVideoContextError::AllocAVHardwareFrameContextError);
                }

                let size = size.expect("encoder needs init hardware frame for size");
                unsafe {
                    let frames_ctx = &mut *((&mut *hw_frames_ctx).data as *mut AVHWFramesContext);
                    frames_ctx.sw_format = AVPixelFormat::AV_PIX_FMT_NV12;
                    frames_ctx.format = AVPixelFormat::AV_PIX_FMT_QSV;
                    frames_ctx.width = size.width as i32;
                    frames_ctx.height = size.height as i32;
                    frames_ctx.initial_pool_size = 5;
                }

                if unsafe { av_hwframe_ctx_init(hw_frames_ctx) } != 0 {
                    return Err(CreateVideoContextError::InitAVHardwareFrameContextError);
                }

                unsafe {
                    context_mut.hw_frames_ctx = av_buffer_ref(hw_frames_ctx);
                }
            }
        } else {
            unsafe {
                context_mut.hw_device_ctx = av_buffer_ref(hw_device_ctx);
            }
        }
    }

    Ok(codec)
}

#[cfg(target_os = "linux")]
pub fn create_video_context(
    context: &mut *mut AVCodecContext,
    kind: CodecType,
) -> Result<*const AVCodec, CreateVideoContextError> {
    let codec = unsafe { kind.find_av_codec() };
    if codec.is_null() {
        return Err(CreateVideoContextError::NotFoundAVCodec);
    }

    *context = unsafe { avcodec_alloc_context3(codec) };
    if context.is_null() {
        return Err(CreateVideoContextError::AllocAVContextError);
    }

    Ok(codec)
}

#[cfg(target_os = "macos")]
pub fn create_video_context(
    context: &mut *mut AVCodecContext,
    kind: CodecType,
    size: Option<Size>,
) -> Result<*const AVCodec, CreateVideoContextError> {
    let codec = unsafe { kind.find_av_codec() };
    if codec.is_null() {
        return Err(CreateVideoContextError::NotFoundAVCodec);
    }

    *context = unsafe { avcodec_alloc_context3(codec) };
    if context.is_null() {
        return Err(CreateVideoContextError::AllocAVContextError);
    }

    if kind.is_hardware() {
        let mut hw_device_ctx = std::ptr::null_mut();
        if unsafe {
            av_hwdevice_ctx_create(
                &mut hw_device_ctx,
                AVHWDeviceType::AV_HWDEVICE_TYPE_VIDEOTOOLBOX,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
            )
        } != 0
        {
            return Err(CreateVideoContextError::InitAVHardwareDeviceContextError);
        }

        let context_mut = unsafe { &mut **context };
        context_mut.hw_device_ctx = unsafe { av_buffer_ref(hw_device_ctx) };

        if kind.is_encoder() {
            let hw_frames_ctx = unsafe { av_hwframe_ctx_alloc(context_mut.hw_device_ctx) };
            if hw_frames_ctx.is_null() {
                return Err(CreateVideoContextError::AllocAVHardwareFrameContextError);
            }

            let size = size.expect("encoder needs init hardware frame for size");
            unsafe {
                let frames_ctx = &mut *((&mut *hw_frames_ctx).data as *mut AVHWFramesContext);
                frames_ctx.sw_format = AVPixelFormat::AV_PIX_FMT_NV12;
                frames_ctx.format = AVPixelFormat::AV_PIX_FMT_VIDEOTOOLBOX;
                frames_ctx.width = size.width as i32;
                frames_ctx.height = size.height as i32;
                frames_ctx.initial_pool_size = 5;
            }

            if unsafe { av_hwframe_ctx_init(hw_frames_ctx) } != 0 {
                return Err(CreateVideoContextError::InitAVHardwareFrameContextError);
            }

            unsafe {
                context_mut.hw_frames_ctx = av_buffer_ref(hw_frames_ctx);
            }
        }
    }

    Ok(codec)
}

#[derive(Error, Debug)]
pub enum CreateVideoFrameError {
    #[error("failed to alloc av frame")]
    AllocAVFrameError,
    #[error("failed to alloc hardware av frame buffer")]
    AllocHardwareAVFrameBufferError,
    #[error("failed to alloc av frame buffer")]
    AllocAVFrameBufferError,
}

pub fn create_video_frame(
    frame: &mut *mut AVFrame,
    context: *const AVCodecContext,
) -> Result<(), CreateVideoFrameError> {
    *frame = unsafe { av_frame_alloc() };
    if frame.is_null() {
        return Err(CreateVideoFrameError::AllocAVFrameError);
    }

    let context_ref = unsafe { &*context };
    let frame_mut = unsafe { &mut **frame };

    frame_mut.width = context_ref.width;
    frame_mut.height = context_ref.height;
    frame_mut.format = context_ref.pix_fmt as i32;

    // qsv needs to indicate the use of hardware textures, otherwise qsv will return
    // software textures.
    if !context_ref.hw_device_ctx.is_null() {
        if unsafe { av_hwframe_get_buffer(context_ref.hw_frames_ctx, *frame, 0) } != 0 {
            return Err(CreateVideoFrameError::AllocHardwareAVFrameBufferError);
        }
    } else {
        if unsafe { av_frame_get_buffer(*frame, 0) } != 0 {
            return Err(CreateVideoFrameError::AllocAVFrameBufferError);
        }
    }

    Ok(())
}

pub fn set_option(context: &mut AVCodecContext, key: &str, value: i64) {
    unsafe {
        av_opt_set_int(context.priv_data, c_str!(key), value, 0);
    }
}

pub fn set_str_option(context: &mut AVCodecContext, key: &str, value: &str) {
    unsafe {
        av_opt_set(context.priv_data, c_str!(key), c_str!(value), 0);
    }
}
