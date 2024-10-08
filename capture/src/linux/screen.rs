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
    #[error("failed to create sw scale context")]
    CreateSWScaleContextError,
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
                frame.format = VideoFormat::NV12;
                frame.sub_format = VideoSubFormat::SW;

                while let Some(avframe) = capture.read() {
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
        unsafe {
            avdevice_register_all();
        }

        let mut this = Self {
            packet: unsafe { av_packet_alloc() },
            frame: unsafe { av_frame_alloc() },
            scaled_frame: unsafe { av_frame_alloc() },
            sws_ctx: null_mut(),
            codec_ctx: null_mut(),
            fmt_ctx: null_mut(),
        };

        let format = unsafe { av_find_input_format(c_str!("kmsgrab")) };
        if format.is_null() {
            return Err(ScreenCaptureError::NotFoundInputFormat);
        }

        if unsafe {
            avformat_open_input(
                &mut this.fmt_ctx,
                c_str!("/dev/dri/card0"),
                format,
                null_mut(),
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

        this.sws_ctx = unsafe {
            sws_getContext(
                (&*stream.codecpar).width,
                (&*stream.codecpar).height,
                std::mem::transmute((&*stream.codecpar).format),
                options.size.width as i32,
                options.size.height as i32,
                AVPixelFormat::AV_PIX_FMT_NV12,
                SWS_BILINEAR,
                null_mut(),
                null_mut(),
                null(),
            )
        };

        if this.sws_ctx.is_null() {
            return Err(ScreenCaptureError::CreateSWScaleContextError);
        }

        unsafe {
            let frame_mut = &mut *this.scaled_frame;
            frame_mut.format = AVPixelFormat::AV_PIX_FMT_NV12 as i32;
            frame_mut.width = options.size.width as i32;
            frame_mut.height = options.size.height as i32;

            av_image_alloc(
                frame_mut.data.as_mut_ptr(),
                frame_mut.linesize.as_mut_ptr(),
                frame_mut.width,
                frame_mut.height,
                AVPixelFormat::AV_PIX_FMT_NV12,
                32,
            );
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
    }
}
