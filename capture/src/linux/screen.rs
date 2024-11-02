use crate::{CaptureHandler, FrameArrived, Source, SourceType, VideoCaptureSourceDescription};

use std::{
    ptr::{null, null_mut},
    sync::{atomic::AtomicBool, Arc},
    thread::{self, sleep},
    time::Duration,
};

use hylarana_common::{
    atomic::EasyAtomic,
    c_str,
    frame::{VideoFormat, VideoFrame, VideoSubFormat},
};

use mirror_ffmpeg_sys::*;
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
    #[error("failed to create sw scale context")]
    CreateSWScaleContextError,
}

#[derive(Default)]
pub struct ScreenCapture(Arc<AtomicBool>);

impl CaptureHandler for ScreenCapture {
    type Frame = VideoFrame;
    type Error = ScreenCaptureError;
    type CaptureDescriptor = VideoCaptureSourceDescription;

    // x11 Capture does not currently support multiple screens.
    fn get_sources() -> Result<Vec<Source>, Self::Error> {
        Ok(vec![Source {
            index: 0,
            is_default: true,
            kind: SourceType::Screen,
            id: ":0.0".to_string(),
            name: "default display".to_string(),
        }])
    }

    fn start<S: FrameArrived<Frame = Self::Frame> + 'static>(
        &self,
        options: Self::CaptureDescriptor,
        mut arrived: S,
    ) -> Result<(), Self::Error> {
        let mut capture = Capture::new(&options)?;

        let status = Arc::downgrade(&self.0);
        self.0.update(true);

        thread::Builder::new()
            .name("LinuxScreenCaptureThread".to_string())
            .spawn(move || {
                let mut frame = VideoFrame::default();
                frame.width = options.size.width;
                frame.height = options.size.height;
                frame.sub_format = VideoSubFormat::SW;
                frame.format = VideoFormat::NV12;

                while let Some(avframe) = capture.read() {
                    if let Some(status) = status.upgrade() {
                        if !status.get() {
                            break;
                        }
                    } else {
                        break;
                    }

                    let format = unsafe { std::mem::transmute::<_, AVPixelFormat>(avframe.format) };
                    match format {
                        AVPixelFormat::AV_PIX_FMT_NV12 => {
                            for i in 0..2 {
                                frame.data[i] = avframe.data[i] as _;
                                frame.linesize[i] = avframe.linesize[i] as usize;
                            }

                            if !arrived.sink(&frame) {
                                break;
                            }
                        }
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
    sws_ctx: *mut SwsContext,
    packet: *mut AVPacket,
    frame: *mut AVFrame,
    scaled_frame: *mut AVFrame,
}

unsafe impl Send for Capture {}
unsafe impl Sync for Capture {}

impl Capture {
    fn new(options: &VideoCaptureSourceDescription) -> Result<Self, ScreenCaptureError> {
        let mut this = Self {
            packet: unsafe { av_packet_alloc() },
            frame: unsafe { av_frame_alloc() },
            scaled_frame: unsafe { av_frame_alloc() },
            sws_ctx: null_mut(),
            codec_ctx: null_mut(),
            fmt_ctx: null_mut(),
        };

        // Currently you can only capture the screen in the x11 desktop environment.
        let format = unsafe { av_find_input_format(c_str!("x11grab")) };
        if format.is_null() {
            return Err(ScreenCaptureError::NotFoundInputFormat);
        }

        // It's just in BGRA format, which is probably all that's available in the x11
        // desktop environment.
        let mut format_options = null_mut();
        for (k, v) in [
            ("pix_fmt".to_string(), "bgr0".to_string()),
            ("framerete".to_string(), options.fps.to_string()),
        ] {
            unsafe {
                av_dict_set(&mut format_options, c_str!(k), c_str!(v), 0);
            }
        }

        if unsafe {
            avformat_open_input(
                &mut this.fmt_ctx,
                c_str!(options.source.id.as_str()),
                format,
                &mut format_options,
            )
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

        // Desktop capture generally has only one stream.
        let streams = unsafe { std::slice::from_raw_parts(ctx_ref.streams, 1) };
        let stream = unsafe { &*(streams[0]) };
        let codecpar = unsafe { &*stream.codecpar };

        let codec = unsafe { avcodec_find_decoder(codecpar.codec_id) };
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

        let scale_frame_mut = unsafe { &mut *this.scaled_frame };
        unsafe {
            scale_frame_mut.format = AVPixelFormat::AV_PIX_FMT_NV12 as i32;
            scale_frame_mut.width = options.size.width as i32;
            scale_frame_mut.height = options.size.height as i32;

            av_image_alloc(
                scale_frame_mut.data.as_mut_ptr(),
                scale_frame_mut.linesize.as_mut_ptr(),
                scale_frame_mut.width,
                scale_frame_mut.height,
                std::mem::transmute(scale_frame_mut.format),
                32,
            );
        }

        // The captured frames are in BGRA format and need to be converted to NV12 and
        // also scaled to match the output resolution.
        this.sws_ctx = unsafe {
            sws_getContext(
                codecpar.width,
                codecpar.height,
                AVPixelFormat::AV_PIX_FMT_BGR0,
                options.size.width as i32,
                options.size.height as i32,
                std::mem::transmute(scale_frame_mut.format),
                SWS_FAST_BILINEAR,
                null_mut(),
                null_mut(),
                null(),
            )
        };

        if this.sws_ctx.is_null() {
            return Err(ScreenCaptureError::CreateSWScaleContextError);
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

        unsafe {
            let frame_mut = &mut *self.frame;
            let scaled_frame_mut = &mut *self.scaled_frame;
            sws_scale(
                self.sws_ctx,
                frame_mut.data.as_ptr() as _,
                frame_mut.linesize.as_ptr(),
                0,
                frame_mut.height,
                scaled_frame_mut.data.as_mut_ptr(),
                scaled_frame_mut.linesize.as_mut_ptr(),
            );
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

        if !self.scaled_frame.is_null() {
            unsafe {
                av_frame_free(&mut self.scaled_frame);
            }
        }

        if !self.sws_ctx.is_null() {
            unsafe {
                sws_freeContext(self.sws_ctx);
            }
        }
    }
}
