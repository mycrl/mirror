#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoDecoderType {
    H264,
    D3D11,
    Qsv,
    Cuda,
}

impl Into<&'static str> for VideoDecoderType {
    fn into(self) -> &'static str {
        match self {
            Self::H264 => "h264",
            Self::D3D11 => "d3d11va",
            Self::Qsv => "h264_qsv",
            Self::Cuda => "h264_cuvid",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoEncoderType {
    X264,
    Qsv,
    Cuda,
}

impl Into<&'static str> for VideoEncoderType {
    fn into(self) -> &'static str {
        match self {
            Self::X264 => "libx264",
            Self::Qsv => "h264_qsv",
            Self::Cuda => "h264_nvenc",
        }
    }
}

pub struct VideoDecoderSettings {
    pub codec: VideoDecoderType,
}

pub struct VideoDecoder {
    
}

impl VideoDecoder {
    pub fn new() {

    }
}

mod util {
    use crate::{VideoDecoderType, VideoEncoderType};

    use ffmpeg_sys_next::*;
    use utils::strings::Strings;

    #[derive(Clone, Copy)]
    pub enum CodecType {
        Encoder(VideoEncoderType),
        Decoder(VideoDecoderType),
    }

    impl CodecType {
        fn is_qsv(self) -> bool {
            match self {
                CodecType::Encoder(kind) => kind == VideoEncoderType::Qsv,
                CodecType::Decoder(kind) => kind == VideoDecoderType::Qsv,
            }
        }

        fn is_d3d(self) -> bool {
            match self {
                CodecType::Decoder(kind) => kind == VideoDecoderType::D3D11,
                _ => false,
            }
        }
    }

    pub struct HardwareFrameSize {
        pub width: u32,
        pub height: u32,
    }

    #[repr(C)]
    struct AVD3D11VADeviceContext {
        ID3D11Device        *device,
  
     /**
      * If unset, this will be set from the device field on init.
      *
      * Deallocating the AVHWDeviceContext will always release this interface,
      * and it does not matter whether it was user-allocated.
      */
     ID3D11DeviceContext *device_context;
  
     /**
      * If unset, this will be set from the device field on init.
      *
      * Deallocating the AVHWDeviceContext will always release this interface,
      * and it does not matter whether it was user-allocated.
      */
     ID3D11VideoDevice   *video_device;
  
     /**
      * If unset, this will be set from the device_context field on init.
      *
      * Deallocating the AVHWDeviceContext will always release this interface,
      * and it does not matter whether it was user-allocated.
      */
     ID3D11VideoContext  *video_context;
  
     /**
      * Callbacks for locking. They protect accesses to device_context and
      * video_context calls. They also protect access to the internal staging
      * texture (for av_hwframe_transfer_data() calls). They do NOT protect
      * access to hwcontext or decoder state in general.
      *
      * If unset on init, the hwcontext implementation will set them to use an
      * internal mutex.
      *
      * The underlying lock must be recursive. lock_ctx is for free use by the
      * locking implementation.
      */
     void (*lock)(void *lock_ctx);
     void (*unlock)(void *lock_ctx);
     void *lock_ctx;
    }

    pub fn create_video_context(kind: CodecType, frame_size: Option<HardwareFrameSize>) -> Option<()> {
        let codec = match kind {
            CodecType::Encoder(kind) => {
                let codec: &str = kind.into();
                unsafe { avcodec_find_encoder_by_name(Strings::from(codec).as_ptr()) }
            }
            CodecType::Decoder(kind) => {
                if kind == VideoDecoderType::D3D11 {
                    unsafe {
                        avcodec_find_decoder(AVCodecID::AV_CODEC_ID_H264)
                    }
                } else {
                    let codec: &str = kind.into();
                    unsafe { 
                        avcodec_find_decoder_by_name(Strings::from(codec).as_ptr())
                    }
                }
            }
        };

        if codec.is_null() {
            return None;
        }

        let context = unsafe { avcodec_alloc_context3(codec) };
        if context.is_null() {
            return None;
        }

        if kind.is_d3d() || kind.is_qsv() {
            let hw_device_ctx = unsafe { av_hwdevice_ctx_alloc(AVHWDeviceType::AV_HWDEVICE_TYPE_D3D11VA) };
            if hw_device_ctx.is_null() {
                return None;
            }

            let d3d11_hwctx = unsafe {
                let hwctx = (&mut *hw_device_ctx).data as *mut AVHWDeviceContext;
                (&mut *hwctx).hwctx as *mut AVD3D11VADeviceContext
            };
        }   

        None
    }
}
