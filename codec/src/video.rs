use std::{ffi::c_int, ptr::null_mut};

use ffmpeg_sys_next::*;
use frame::{VideoFormat, VideoFrame};
use thiserror::Error;

#[cfg(target_os = "windows")]
use utils::win32::Direct3DDevice;

use crate::{
    util::{
        create_video_context, create_video_frame, set_option, set_str_option, CodecType,
        CreateVideoContextDescriptor, CreateVideoContextError, CreateVideoFrameError,
        HardwareFrameSize,
    },
    CodecError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoDecoderType {
    D3D11,
    Qsv,
    Cuda,
}

impl Into<&'static str> for VideoDecoderType {
    fn into(self) -> &'static str {
        match self {
            Self::D3D11 => "d3d11va",
            Self::Qsv => "h264_qsv",
            Self::Cuda => "h264_cuvid",
        }
    }
}

impl TryFrom<&str> for VideoDecoderType {
    type Error = CodecError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value {
            "d3d11va" => Self::D3D11,
            "h264_qsv" => Self::Qsv,
            "h264_cuvid" => Self::Cuda,
            _ => return Err(CodecError::NotSupportCodec),
        })
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

impl TryFrom<&str> for VideoEncoderType {
    type Error = CodecError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value {
            "libx264" => Self::X264,
            "h264_qsv" => Self::Qsv,
            "h264_cuvid" => Self::Cuda,
            _ => return Err(CodecError::NotSupportCodec),
        })
    }
}

#[derive(Debug, Clone)]
pub struct VideoDecoderSettings {
    /// Name of the codec implementation.
    ///
    /// The name is globally unique among encoders and among decoders (but
    /// an encoder and a decoder can share the same name). This is
    /// the primary way to find a codec from the user perspective.
    pub codec: VideoDecoderType,
    #[cfg(target_os = "windows")]
    pub direct3d: Option<Direct3DDevice>,
}

#[derive(Error, Debug)]
pub enum VideoDecoderError {
    #[error(transparent)]
    CreateVideoContextError(#[from] CreateVideoContextError),
    #[error(transparent)]
    CreateVideoFrameError(#[from] CreateVideoFrameError),
    #[error("failed to open av codec")]
    OpenAVCodecError,
    #[error("failed to init av parser context")]
    InitAVCodecParserContextError,
    #[error("failed to alloc av packet")]
    AllocAVPacketError,
    #[error("parser parse packet failed")]
    ParsePacketError,
    #[error("send av packet to codec failed")]
    SendPacketToAVCodecError,
    #[error("failed to alloc av frame")]
    AllocAVFrameError,
}

pub struct VideoDecoder {
    context: *mut AVCodecContext,
    parser: *mut AVCodecParserContext,
    packet: *mut AVPacket,
    av_frame: *mut AVFrame,
    frame: VideoFrame,
}

unsafe impl Sync for VideoDecoder {}
unsafe impl Send for VideoDecoder {}

impl VideoDecoder {
    pub fn new(options: VideoDecoderSettings) -> Result<Self, VideoDecoderError> {
        let mut this = Self {
            context: null_mut(),
            parser: null_mut(),
            packet: null_mut(),
            av_frame: null_mut(),
            frame: VideoFrame::default(),
        };

        let codec = create_video_context(CreateVideoContextDescriptor {
            kind: CodecType::Decoder(options.codec),
            context: &mut this.context,
            frame_size: None,
            #[cfg(target_os = "windows")]
            direct3d: options.direct3d,
        })?;

        let context_mut = unsafe { &mut *this.context };
        context_mut.delay = 0;
        context_mut.max_samples = 1;
        context_mut.has_b_frames = 0;
        context_mut.skip_alpha = true as i32;
        context_mut.flags |= AV_CODEC_FLAG_LOW_DELAY as i32;
        context_mut.flags2 |= AV_CODEC_FLAG2_FAST;
        context_mut.hwaccel_flags |= AV_HWACCEL_FLAG_IGNORE_LEVEL | AV_HWACCEL_FLAG_UNSAFE_OUTPUT;

        if options.codec == VideoDecoderType::Qsv {
            set_option(context_mut, "async_depth", 1);
        }

        if unsafe { avcodec_open2(this.context, codec, null_mut()) } != 0 {
            return Err(VideoDecoderError::OpenAVCodecError);
        }

        if unsafe { avcodec_is_open(this.context) } == 0 {
            return Err(VideoDecoderError::OpenAVCodecError);
        }

        this.parser = unsafe { av_parser_init({ &*codec }.id as i32) };
        if this.parser.is_null() {
            return Err(VideoDecoderError::InitAVCodecParserContextError);
        }

        this.packet = unsafe { av_packet_alloc() };
        if this.packet.is_null() {
            return Err(VideoDecoderError::AllocAVPacketError);
        }

        Ok(this)
    }

    pub fn decode(&mut self, mut buf: &[u8], pts: u64) -> Result<(), VideoDecoderError> {
        if buf.is_empty() {
            return Ok(());
        }

        let mut size = buf.len();
        while size > 0 {
            let packet = unsafe { &mut *self.packet };
            let len = unsafe {
                av_parser_parse2(
                    self.parser,
                    self.context,
                    &mut packet.data,
                    &mut packet.size,
                    buf.as_ptr(),
                    buf.len() as c_int,
                    pts as i64,
                    AV_NOPTS_VALUE,
                    0,
                )
            };

            // When parsing the code stream, an abnormal return code appears and processing
            // should not be continued.
            if len < 0 {
                return Err(VideoDecoderError::ParsePacketError);
            }

            let len = len as usize;
            buf = &buf[len..];
            size -= len;

            // One or more cells have been parsed.
            if packet.size > 0 {
                if unsafe { avcodec_send_packet(self.context, self.packet) } != 0 {
                    return Err(VideoDecoderError::SendPacketToAVCodecError);
                }
            }
        }

        Ok(())
    }

    pub fn read<'a>(&'a mut self) -> Option<&'a VideoFrame> {
        // When decoding, each video frame uses a newly created one.
        if !self.av_frame.is_null() {
            unsafe {
                av_frame_free(&mut self.av_frame);
            }
        }

        self.av_frame = unsafe { av_frame_alloc() };
        if self.av_frame.is_null() {
            return None;
        }

        if unsafe { avcodec_receive_frame(self.context, self.av_frame) } != 0 {
            return None;
        }

        let frame = unsafe { &*self.av_frame };
        self.frame.width = frame.width as u32;
        self.frame.height = frame.height as u32;

        match unsafe { std::mem::transmute(frame.format) } {
            // mfxFrameSurface1.Data.MemId contains a pointer to the mfxHDLPair structure
            // when importing the following frames as QSV frames:
            //
            // VAAPI: mfxHDLPair.first contains a VASurfaceID pointer. mfxHDLPair.second is
            // always MFX_INFINITE.
            //
            // DXVA2: mfxHDLPair.first contains IDirect3DSurface9 pointer. mfxHDLPair.second
            // is always MFX_INFINITE.
            //
            // D3D11: mfxHDLPair.first contains a ID3D11Texture2D pointer. mfxHDLPair.second
            // contains the texture array index of the frame if the ID3D11Texture2D is an
            // array texture, or always MFX_INFINITE if it is a normal texture.
            #[cfg(target_os = "windows")]
            AVPixelFormat::AV_PIX_FMT_QSV => {
                let surface = unsafe { &*(frame.data[3] as *const mfxFrameSurface1) };
                let hdl = unsafe { &*(surface.Data.MemId as *const mfxHDLPair) };

                self.frame.data[0] = hdl.first;
                self.frame.data[1] = hdl.second;

                self.frame.hardware = true;
                self.frame.format = VideoFormat::NV12;
            }
            // The d3d11va video frame texture has no stride.
            AVPixelFormat::AV_PIX_FMT_D3D11 => {
                for i in 0..2 {
                    self.frame.data[i] = frame.data[i] as *const _;
                }

                self.frame.hardware = true;
                self.frame.format = VideoFormat::NV12;
            }
            AVPixelFormat::AV_PIX_FMT_YUV420P => {
                for i in 0..3 {
                    self.frame.data[i] = frame.data[i] as *const _;
                    self.frame.linesize[i] = frame.linesize[i] as usize;
                }

                self.frame.hardware = false;
                self.frame.format = VideoFormat::I420;
            }
            _ => unimplemented!("not supports the video frame format!"),
        };

        Some(&self.frame)
    }
}

impl Drop for VideoDecoder {
    fn drop(&mut self) {
        if !self.packet.is_null() {
            unsafe {
                av_packet_free(&mut self.packet);
            }
        }

        if !self.parser.is_null() {
            unsafe {
                av_parser_close(self.parser);
            }
        }

        if !self.context.is_null() {
            let ctx_mut = unsafe { &mut *self.context };
            if !ctx_mut.hw_device_ctx.is_null() {
                unsafe {
                    av_buffer_unref(&mut ctx_mut.hw_device_ctx);
                }
            }

            if !ctx_mut.hw_frames_ctx.is_null() {
                unsafe {
                    av_buffer_unref(&mut ctx_mut.hw_frames_ctx);
                }
            }

            unsafe {
                avcodec_free_context(&mut self.context);
            }
        }

        if !self.av_frame.is_null() {
            unsafe {
                av_frame_free(&mut self.av_frame);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct VideoEncoderSettings {
    /// Name of the codec implementation.
    ///
    /// The name is globally unique among encoders and among decoders (but an
    /// encoder and a decoder can share the same name). This is the primary way
    /// to find a codec from the user perspective.
    pub codec: VideoEncoderType,
    pub frame_rate: u8,
    /// picture width / height
    pub width: u32,
    /// picture width / height
    pub height: u32,
    /// the average bitrate
    pub bit_rate: u64,
    /// the number of pictures in a group of pictures, or 0 for intra_only
    pub key_frame_interval: u32,
    #[cfg(target_os = "windows")]
    pub direct3d: Option<Direct3DDevice>,
}

#[derive(Error, Debug)]
pub enum VideoEncoderError {
    #[error(transparent)]
    CreateVideoContextError(#[from] CreateVideoContextError),
    #[error(transparent)]
    CreateVideoFrameError(#[from] CreateVideoFrameError),
    #[error("failed to open av codec")]
    OpenAVCodecError,
    #[error("failed to alloc av packet")]
    AllocAVPacketError,
    #[error("send frame to codec failed")]
    EncodeFrameError,
}

pub struct VideoEncoder {
    context: *mut AVCodecContext,
    packet: *mut AVPacket,
    frame: *mut AVFrame,
    initialized: bool,
}

unsafe impl Sync for VideoEncoder {}
unsafe impl Send for VideoEncoder {}

impl VideoEncoder {
    pub fn new(options: VideoEncoderSettings) -> Result<Self, VideoEncoderError> {
        let mut this = Self {
            context: null_mut(),
            packet: null_mut(),
            frame: null_mut(),
            initialized: false,
        };

        let codec = create_video_context(CreateVideoContextDescriptor {
            kind: CodecType::Encoder(options.codec),
            context: &mut this.context,
            frame_size: Some(HardwareFrameSize {
                width: options.width,
                height: options.height,
            }),
            #[cfg(target_os = "windows")]
            direct3d: options.direct3d,
        })?;

        let context_mut = unsafe { &mut *this.context };
        context_mut.delay = 0;
        context_mut.max_samples = 1;
        context_mut.has_b_frames = 0;
        context_mut.max_b_frames = 0;
        context_mut.flags2 |= AV_CODEC_FLAG2_FAST;
        context_mut.flags |= AV_CODEC_FLAG_LOW_DELAY as i32 | AV_CODEC_FLAG_GLOBAL_HEADER as i32;
        context_mut.profile = FF_PROFILE_H264_BASELINE;

        // The QSV encoder can only use qsv frames. Although the internal structure is a
        // platform-specific hardware texture, you cannot directly tell qsv a specific
        // format.
        if options.codec == VideoEncoderType::Qsv {
            context_mut.pix_fmt = AVPixelFormat::AV_PIX_FMT_QSV;
        } else {
            context_mut.thread_count = 4;
            context_mut.thread_type = FF_THREAD_SLICE;
            context_mut.pix_fmt = AVPixelFormat::AV_PIX_FMT_NV12;
        }

        // The bitrate of qsv is always too high, so if it is qsv, using half of the
        // current base bitrate is enough.
        let mut bit_rate = options.bit_rate as i64;
        if options.codec == VideoEncoderType::Qsv {
            bit_rate = bit_rate / 2;
        }

        context_mut.bit_rate = bit_rate;
        context_mut.rc_max_rate = bit_rate;
        context_mut.rc_buffer_size = bit_rate as i32;
        context_mut.bit_rate_tolerance = bit_rate as i32;
        context_mut.rc_initial_buffer_occupancy = (bit_rate * 3 / 4) as i32;
        context_mut.framerate = unsafe { av_make_q(options.frame_rate as i32, 1) };
        context_mut.time_base = unsafe { av_make_q(1, options.frame_rate as i32) };
        context_mut.pkt_timebase = unsafe { av_make_q(1, options.frame_rate as i32) };
        context_mut.gop_size = options.key_frame_interval as i32 / 2;
        context_mut.height = options.height as i32;
        context_mut.width = options.width as i32;

        match options.codec {
            VideoEncoderType::Qsv => {
                set_option(context_mut, "async_depth", 1);
                set_option(context_mut, "low_power", 1);
                set_option(context_mut, "vcm", 1);
            }
            VideoEncoderType::Cuda => {
                set_option(context_mut, "zerolatency", 1);
                set_option(context_mut, "b_adapt", 0);
                set_option(context_mut, "rc", 2);
                set_option(context_mut, "cbr", 1);
                set_option(context_mut, "preset", 7);
                set_option(context_mut, "tune", 3);
            }
            VideoEncoderType::X264 => {
                set_str_option(context_mut, "preset", "superfast");
                set_str_option(context_mut, "tune", "zerolatency");
                set_option(context_mut, "nal-hrd", 2);
                set_option(
                    context_mut,
                    "sc_threshold",
                    options.key_frame_interval as i64,
                );
            }
        };

        if unsafe { avcodec_open2(this.context, codec, null_mut()) } != 0 {
            return Err(VideoEncoderError::OpenAVCodecError);
        }

        if unsafe { avcodec_is_open(this.context) } == 0 {
            return Err(VideoEncoderError::OpenAVCodecError);
        }

        this.packet = unsafe { av_packet_alloc() };
        if this.packet.is_null() {
            return Err(VideoEncoderError::AllocAVPacketError);
        }

        // When encoding a video, frames can be reused. Here, a frame is created and
        // then reused by replacing the data inside the frame.
        create_video_frame(
            &mut this.frame,
            this.context,
            CodecType::Encoder(options.codec),
        )?;

        Ok(this)
    }

    pub fn update(&mut self, frame: &VideoFrame) -> bool {
        let av_frame = unsafe { &mut *self.frame };
        if frame.hardware {
            // mfxFrameSurface1.Data.MemId contains a pointer to the mfxHDLPair structure
            // when importing the following frames as QSV frames:
            //
            // VAAPI: mfxHDLPair.first contains a VASurfaceID pointer. mfxHDLPair.second is
            // always MFX_INFINITE.
            //
            // DXVA2: mfxHDLPair.first contains IDirect3DSurface9 pointer. mfxHDLPair.second
            // is always MFX_INFINITE.
            //
            // D3D11: mfxHDLPair.first contains a ID3D11Texture2D pointer. mfxHDLPair.second
            // contains the texture array index of the frame if the ID3D11Texture2D is an
            // array texture, or always MFX_INFINITE if it is a normal texture.
            #[cfg(target_os = "windows")]
            if av_frame.format == AVPixelFormat::AV_PIX_FMT_QSV as i32 {
                let surface = unsafe { &mut *(av_frame.data[3] as *mut mfxFrameSurface1) };
                let hdl = unsafe { &mut *(surface.Data.MemId as *mut mfxHDLPair) };

                hdl.first = frame.data[0] as *mut _;
                hdl.second = frame.data[1] as *mut _;
            }
        } else {
            // Anyway, the hardware encoder has no way to check whether the current frame is
            // writable.
            if unsafe { av_frame_make_writable(self.frame) } != 0 {
                return false;
            }

            // Directly replacing the pointer may cause some problems with pointer access.
            // Copying data to the frame is the safest way.
            unsafe {
                av_image_copy(
                    av_frame.data.as_mut_ptr(),
                    av_frame.linesize.as_mut_ptr(),
                    frame.data.as_ptr() as *const _,
                    [
                        frame.linesize[0] as i32,
                        frame.linesize[1] as i32,
                        frame.linesize[2] as i32,
                    ]
                    .as_ptr(),
                    { &*self.context }.pix_fmt,
                    av_frame.width,
                    av_frame.height,
                );
            }
        }

        true
    }

    pub fn encode(&mut self) -> Result<(), VideoEncoderError> {
        let av_frame = unsafe { &mut *self.frame };
        av_frame.pts = unsafe {
            let context_ref = &*self.context;
            av_rescale_q(
                context_ref.frame_num,
                context_ref.pkt_timebase,
                context_ref.time_base,
            )
        };

        if unsafe { avcodec_send_frame(self.context, self.frame) } != 0 {
            return Err(VideoEncoderError::EncodeFrameError);
        }

        Ok(())
    }

    pub fn read<'a>(&'a mut self) -> Option<(&'a [u8], i32, u64)> {
        let packet_ref = unsafe { &*self.packet };
        let context_ref = unsafe { &*self.context };

        // In the global header mode, ffmpeg will not automatically add sps and pps to
        // the bitstream. You need to manually extract the sps and pps data and add it
        // to the header as configuration information.
        if !self.initialized {
            self.initialized = true;

            return Some((
                unsafe {
                    std::slice::from_raw_parts(
                        context_ref.extradata,
                        context_ref.extradata_size as usize,
                    )
                },
                2,
                packet_ref.pts as u64,
            ));
        }

        if unsafe { avcodec_receive_packet(self.context, self.packet) } != 0 {
            return None;
        }

        Some((
            unsafe { std::slice::from_raw_parts(packet_ref.data, packet_ref.size as usize) },
            packet_ref.flags,
            packet_ref.pts as u64,
        ))
    }
}

impl Drop for VideoEncoder {
    fn drop(&mut self) {
        if !self.packet.is_null() {
            unsafe {
                av_packet_free(&mut self.packet);
            }
        }

        if !self.context.is_null() {
            let ctx_mut = unsafe { &mut *self.context };
            if !ctx_mut.hw_device_ctx.is_null() {
                unsafe {
                    av_buffer_unref(&mut ctx_mut.hw_device_ctx);
                }
            }

            if !ctx_mut.hw_frames_ctx.is_null() {
                unsafe {
                    av_buffer_unref(&mut ctx_mut.hw_frames_ctx);
                }
            }

            unsafe {
                avcodec_free_context(&mut self.context);
            }
        }

        if !self.frame.is_null() {
            unsafe {
                av_frame_free(&mut self.frame);
            }
        }
    }
}
