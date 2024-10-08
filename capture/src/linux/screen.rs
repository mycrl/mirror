use crate::{CaptureHandler, FrameArrived, Source, SourceType, VideoCaptureSourceDescription};

use std::{
    ptr::{null, null_mut},
    sync::{atomic::AtomicBool, Arc},
    thread::{self, sleep},
    time::Duration,
};

use common::{
    atomic::EasyAtomic,
    c_str,
    frame::{VideoFormat, VideoFrame, VideoSubFormat},
};

use ffmpeg_sys_next::*;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScreenCaptureError {
    #[error(transparent)]
    CreateThreadError(#[from] std::io::Error),
    #[error("not create hardware device context")]
    CreateHWDeviceContextError,
    #[error("not create hardware frame context")]
    CreateHWFrameContextError,
    #[error("not found input format")]
    NotFoundInputFormat,
    #[error("not open input format")]
    NotOpenInputFormat,
    #[error("not open input stream")]
    NotFoundInputStream,
    #[error("not found decoder")]
    NotFoundDecoder,
    #[error("failed to create decoder")]
    CreateDecoderError,
    #[error("failed to set parameters to decoder")]
    SetParametersError,
    #[error("not open decoder")]
    NotOpenDecoder,
    #[error("failed to create filter input")]
    CreateFilterInputError,
    #[error("failed to create filter output")]
    CreateFilterOutputError,
    #[error("failed to create filter chain")]
    CreateFilterChainError,
}

#[derive(Default)]
pub struct ScreenCapture(Arc<AtomicBool>);

impl CaptureHandler for ScreenCapture {
    type Frame = VideoFrame;
    type Error = ScreenCaptureError;
    type CaptureDescriptor = VideoCaptureSourceDescription;

    fn get_sources() -> Result<Vec<Source>, Self::Error> {
        Ok(vec![Source {
            index: 0,
            is_default: true,
            kind: SourceType::Screen,
            id: "default display".to_string(),
            name: "default display".to_string(),
        }])
    }

    fn start<S: FrameArrived<Frame = Self::Frame> + 'static>(
        &self,
        options: Self::CaptureDescriptor,
        mut arrived: S,
    ) -> Result<(), Self::Error> {
        let mut capture = Capture::new(&options)?;

        thread::Builder::new()
            .name("LinuxScreenCaptureThread".to_string())
            .spawn(move || {
                let mut frame = VideoFrame::default();
                frame.width = options.size.width;
                frame.height = options.size.height;
                frame.format = VideoFormat::BGRA;
                frame.sub_format = VideoSubFormat::SW;

                while let Some(avframe) = capture.read() {
                    let format = unsafe { std::mem::transmute::<_, AVPixelFormat>(avframe.format) };
                    match format {
                        AVPixelFormat::AV_PIX_FMT_BGR0 => {
                            frame.data[0] = avframe.data[0] as _;
                            frame.linesize[0] = avframe.linesize[0] as usize;

                            // if !arrived.sink(&frame) {
                            //     break;
                            // }
                        }
                        AVPixelFormat::AV_PIX_FMT_DRM_PRIME => {}
                        _ => unimplemented!("not supports capture pix fmt = {:?}", format),
                    }

                    sleep(Duration::from_millis(1000 / options.fps as u64));
                }
            })?;

        Ok(())
    }

    fn stop(&self) -> Result<(), Self::Error> {
        self.0.update(false);
        Ok(())
    }
}

struct Capture {
    fmt_ctx: *mut AVFormatContext,
    codec_ctx: *mut AVCodecContext,
    packet: *mut AVPacket,
    frame: *mut AVFrame,
    scaled_frame: *mut AVFrame,
    filter_graph: *mut AVFilterGraph,
    buffer_ctx: *mut AVFilterContext,
    buffer_sink_ctx: *mut AVFilterContext,
}

unsafe impl Send for Capture {}
unsafe impl Sync for Capture {}

impl Capture {
    fn new(options: &VideoCaptureSourceDescription) -> Result<Self, ScreenCaptureError> {
        unsafe {
            avdevice_register_all();
        }

        let mut this = Self {
            packet: unsafe { av_packet_alloc() },
            frame: unsafe { av_frame_alloc() },
            scaled_frame: unsafe { av_frame_alloc() },
            codec_ctx: null_mut(),
            fmt_ctx: null_mut(),
            filter_graph: unsafe { avfilter_graph_alloc() },
            buffer_ctx: null_mut(),
            buffer_sink_ctx: null_mut(),
        };

        let format = unsafe { av_find_input_format(c_str!("kmsgrab")) };
        if format.is_null() {
            return Err(ScreenCaptureError::NotFoundInputFormat);
        }

        let mut format_options = null_mut();
        // for (k, v) in [
        //     ("pix_fmt", "rgba"),
        //     ("video_size", &format!("{}x{}", size.width, size.height)),
        // ] {
        //     unsafe {
        //         av_dict_set(
        //             &mut options,
        //             Strings::from(k).as_ptr(),
        //             Strings::from(v).as_ptr(),
        //             0,
        //         );
        //     }
        // }

        if unsafe {
            avformat_open_input(&mut this.fmt_ctx, c_str!(":0"), format, &mut format_options)
        } != 0
        {
            return Err(ScreenCaptureError::NotOpenInputFormat);
        }

        if unsafe { avformat_find_stream_info(this.fmt_ctx, null_mut()) } != 0 {
            return Err(ScreenCaptureError::NotFoundInputStream);
        }

        let ctx_ref = unsafe { &*this.fmt_ctx };
        if ctx_ref.nb_streams == 0 {
            return Err(ScreenCaptureError::NotFoundInputStream);
        }

        let streams = unsafe { std::slice::from_raw_parts(ctx_ref.streams, 1) };
        let stream = unsafe { &*(streams[0]) };

        let codec = unsafe { avcodec_find_decoder((&*stream.codecpar).codec_id) };
        if codec.is_null() {
            return Err(ScreenCaptureError::NotFoundDecoder);
        }

        this.codec_ctx = unsafe { avcodec_alloc_context3(codec) };
        if this.codec_ctx.is_null() {
            return Err(ScreenCaptureError::CreateDecoderError);
        }

        if unsafe { avcodec_parameters_to_context(this.codec_ctx, stream.codecpar) } != 0 {
            return Err(ScreenCaptureError::SetParametersError);
        }

        if unsafe { avcodec_open2(this.codec_ctx, codec, null_mut()) } != 0 {
            return Err(ScreenCaptureError::NotOpenDecoder);
        }

        let mut hw_device_ctx = null_mut();
        if unsafe {
            av_hwdevice_ctx_create(
                &mut hw_device_ctx,
                AVHWDeviceType::AV_HWDEVICE_TYPE_VAAPI,
                null_mut(),
                null_mut(),
                0,
            )
        } != 0
        {
            return Err(ScreenCaptureError::CreateHWDeviceContextError);
        }

        {
            let frame_mut = unsafe { &mut *this.frame };
            frame_mut.hw_frames_ctx = unsafe { av_hwframe_ctx_alloc(hw_device_ctx) };
            if frame_mut.hw_frames_ctx.is_null() {
                return Err(ScreenCaptureError::CreateHWFrameContextError);
            }
        }

        if unsafe {
            avfilter_graph_create_filter(
                &mut this.buffer_ctx,
                avfilter_get_by_name(c_str!("buffer")),
                c_str!("in"),
                c_str!(format!(
                    "video_size={}x{}:pix_fmt={}:time_base=1/{}",
                    { &*stream.codecpar }.width,
                    { &*stream.codecpar }.height,
                    AVPixelFormat::AV_PIX_FMT_DRM_PRIME as u32,
                    options.fps,
                )),
                null_mut(),
                this.filter_graph,
            )
        } != 0
        {
            return Err(ScreenCaptureError::CreateFilterInputError);
        }

        if unsafe {
            avfilter_graph_create_filter(
                &mut this.buffer_sink_ctx,
                avfilter_get_by_name(c_str!("buffersink")),
                c_str!("out"),
                null(),
                null_mut(),
                this.filter_graph,
            )
        } != 0
        {
            return Err(ScreenCaptureError::CreateFilterOutputError);
        }

        if unsafe {
            avfilter_graph_parse_ptr(
                this.filter_graph,
                c_str!(format!(
                    "scale_vaapi=w={}:h={}",
                    options.size.width, options.size.height
                )),
                null_mut(),
                null_mut(),
                null_mut(),
            )
        } != 0
        {
            return Err(ScreenCaptureError::CreateFilterChainError);
        }

        if unsafe { avfilter_graph_config(this.filter_graph, null_mut()) } != 0 {
            return Err(ScreenCaptureError::CreateFilterChainError);
        }

        Ok(this)
    }

    fn read(&mut self) -> Option<&AVFrame> {
        if !self.packet.is_null() {
            unsafe {
                av_packet_unref(self.packet);
            }
        }

        if unsafe { av_read_frame(self.fmt_ctx, self.packet) } != 0 {
            return None;
        }

        if unsafe { avcodec_send_packet(self.codec_ctx, self.packet) } != 0 {
            return None;
        }

        if unsafe { avcodec_receive_frame(self.codec_ctx, self.frame) } != 0 {
            return None;
        }

        if unsafe { av_buffersrc_add_frame(self.buffer_ctx, self.frame) } != 0 {
            return None;
        }

        if unsafe { av_buffersink_get_frame(self.buffer_sink_ctx, self.scaled_frame) } != 0 {
            return None;
        }

        Some(unsafe { &*self.scaled_frame })
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        if !self.fmt_ctx.is_null() {
            unsafe {
                avformat_close_input(&mut self.fmt_ctx);
            }
        }

        if !self.codec_ctx.is_null() {
            unsafe {
                avcodec_free_context(&mut self.codec_ctx);
            }
        }

        if !self.packet.is_null() {
            unsafe {
                av_packet_free(&mut self.packet);
            }
        }

        if !self.frame.is_null() {
            unsafe {
                av_frame_free(&mut self.frame);
            }
        }
    }
}
