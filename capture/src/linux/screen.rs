use crate::{
    CaptureHandler, FrameArrived, Size, Source, SourceType, VideoCaptureSourceDescription,
};

use std::{
    ptr::null_mut,
    sync::{atomic::AtomicBool, Arc},
    thread::{self, sleep},
    time::Duration,
};

use anyhow::{anyhow, Result};
use ffmpeg_sys_next::*;
use frame::{VideoFormat, VideoFrame};
use utils::{atomic::EasyAtomic, strings::Strings};

#[derive(Default)]
pub struct ScreenCapture(Arc<AtomicBool>);

impl CaptureHandler for ScreenCapture {
    type Frame = VideoFrame;
    type Error = anyhow::Error;
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
        let mut capture = Capture::new(options.size)?;

        thread::Builder::new()
            .name("LinuxScreenCaptureThread".to_string())
            .spawn(move || {
                let mut frame = VideoFrame::default();
                frame.width = options.size.width;
                frame.height = options.size.height;
                frame.format = VideoFormat::BGRA;
                frame.hardware = false;

                while let Some(avframe) = capture.read() {
                    match unsafe { std::mem::transmute::<_, AVPixelFormat>(avframe.format) } {
                        AVPixelFormat::AV_PIX_FMT_BGR0 => {
                            frame.data[0] = avframe.data[0] as _;
                            frame.linesize[0] = avframe.linesize[0] as usize;

                            if !arrived.sink(&frame) {
                                break;
                            }
                        }
                        _ => unimplemented!("not supports capture pix fmt"),
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
}

unsafe impl Send for Capture {}
unsafe impl Sync for Capture {}

impl Capture {
    fn new(size: Size) -> Result<Self> {
        unsafe {
            avdevice_register_all();
        }

        let mut this = Self {
            packet: unsafe { av_packet_alloc() },
            frame: unsafe { av_frame_alloc() },
            codec_ctx: null_mut(),
            fmt_ctx: null_mut(),
        };

        let format = unsafe { av_find_input_format(Strings::from("x11grab").as_ptr()) };
        if format.is_null() {
            return Err(anyhow!("not find input format"));
        }

        let mut options = null_mut();
        for (k, v) in [
            ("pix_fmt", "rgba"),
            ("video_size", &format!("{}x{}", size.width, size.height)),
        ] {
            unsafe {
                av_dict_set(
                    &mut options,
                    Strings::from(k).as_ptr(),
                    Strings::from(v).as_ptr(),
                    0,
                );
            }
        }

        if unsafe {
            avformat_open_input(
                &mut this.fmt_ctx,
                Strings::from(":0").as_ptr(),
                format,
                &mut options,
            )
        } != 0
        {
            return Err(anyhow!("not open kms device"));
        }

        if unsafe { avformat_find_stream_info(this.fmt_ctx, null_mut()) } != 0 {
            return Err(anyhow!("not found kms device capture stream"));
        }

        let ctx_ref = unsafe { &*this.fmt_ctx };
        if ctx_ref.nb_streams == 0 {
            return Err(anyhow!("not found a capture stream"));
        }

        let streams = unsafe { std::slice::from_raw_parts(ctx_ref.streams, 1) };
        let stream = unsafe { &*(streams[0]) };

        let codec = unsafe { avcodec_find_decoder((&*stream.codecpar).codec_id) };
        if codec.is_null() {
            return Err(anyhow!("not found decoder"));
        }

        this.codec_ctx = unsafe { avcodec_alloc_context3(codec) };
        if this.codec_ctx.is_null() {
            return Err(anyhow!("not alloc decoder context"));
        }

        if unsafe { avcodec_parameters_to_context(this.codec_ctx, stream.codecpar) } != 0 {
            return Err(anyhow!("failed to set params"));
        }

        if unsafe { avcodec_open2(this.codec_ctx, codec, null_mut()) } != 0 {
            return Err(anyhow!("not open decoder"));
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

        Some(unsafe { &*self.frame })
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
