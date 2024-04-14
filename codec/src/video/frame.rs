#![cfg(feature = "frame")]

use std::{
    ffi::{c_int, CString},
    ptr::null_mut,
};

use crate::api;

#[derive(Debug, Clone)]
pub struct VideoEncoderSettings {
    pub codec_name: String,
    pub max_b_frames: u8,
    pub frame_rate: u8,
    pub width: u32,
    pub height: u32,
    pub bit_rate: u64,
    pub key_frame_interval: u32,
}

impl VideoEncoderSettings {
    fn as_raw(&self) -> api::VideoEncoderSettings {
        api::VideoEncoderSettings {
            max_b_frames: self.max_b_frames,
            frame_rate: self.frame_rate,
            width: self.width,
            height: self.height,
            bit_rate: self.bit_rate,
            key_frame_interval: self.key_frame_interval,
            codec_name: CString::new(self.codec_name.as_str()).unwrap().into_raw(),
        }
    }
}

pub struct VideoFrame<'a> {
    pub buffer: [&'a [u8]; 4],
    pub stride: [u32; 4],
}

impl<'a> VideoFrame<'a> {
    pub fn as_raw(&self) -> api::VideoFrame {
        api::VideoFrame {
            buffer: [
                self.buffer[0].as_ptr(),
                self.buffer[1].as_ptr(),
                self.buffer[2].as_ptr(),
                self.buffer[3].as_ptr(),
            ],
            stride: [
                self.stride[0] as c_int,
                self.stride[1] as c_int,
                self.stride[2] as c_int,
                self.stride[3] as c_int,
            ],
        }
    }

    fn from_raw(raw: *const api::VideoFrame, height: u32) -> Self {
        let frame = unsafe { &*raw };
        Self {
            buffer: [
                unsafe {
                    std::slice::from_raw_parts(
                        frame.buffer[0],
                        frame.stride[0] as usize * height as usize,
                    )
                },
                unsafe {
                    std::slice::from_raw_parts(
                        frame.buffer[1],
                        frame.stride[1] as usize * height as usize,
                    )
                },
                &[],
                &[],
            ],
            stride: [
                frame.stride[0] as u32,
                frame.stride[1] as u32,
                frame.stride[2] as u32,
                frame.stride[3] as u32,
            ],
        }
    }
}

#[repr(C)]
pub struct VideoEncodePacket<'a> {
    codec: api::VideoEncoder,
    pub buffer: &'a [u8],
    pub flags: i32,
}

impl Drop for VideoEncodePacket<'_> {
    fn drop(&mut self) {
        unsafe { api::_unref_video_encoder_packet(self.codec) }
    }
}

impl<'a> VideoEncodePacket<'a> {
    fn from_raw(codec: api::VideoEncoder, ptr: *const api::VideoEncodePacket) -> Self {
        let raw = unsafe { &*ptr };
        Self {
            buffer: unsafe { std::slice::from_raw_parts(raw.buffer, raw.len) },
            flags: raw.flags,
            codec,
        }
    }
}

pub struct VideoFrameSenderProcesser {
    codec: api::VideoEncoder,
}

unsafe impl Send for VideoFrameSenderProcesser {}
unsafe impl Sync for VideoFrameSenderProcesser {}

impl VideoFrameSenderProcesser {
    pub fn new(settings: &VideoEncoderSettings) -> Option<Self> {
        let settings = settings.as_raw();
        let codec = unsafe { api::_create_video_encoder(&settings) };
        if !codec.is_null() {
            Some(Self { codec })
        } else {
            None
        }
    }

    pub fn push_frame(&self, frame: &VideoFrame) -> bool {
        unsafe { api::_video_encoder_send_frame(self.codec, &frame.as_raw()) }
    }

    pub fn read_packet(&self) -> Option<VideoEncodePacket> {
        let packet = unsafe { api::_video_encoder_read_packet(self.codec) };
        if !packet.is_null() {
            Some(VideoEncodePacket::from_raw(self.codec, packet))
        } else {
            None
        }
    }
}

impl Drop for VideoFrameSenderProcesser {
    fn drop(&mut self) {
        unsafe { api::_release_video_encoder(self.codec) }
    }
}

pub struct VideoFrameReceiverProcesser {
    codec: api::VideoDecoder,
}

unsafe impl Send for VideoFrameReceiverProcesser {}
unsafe impl Sync for VideoFrameReceiverProcesser {}

impl VideoFrameReceiverProcesser {
    pub fn new(codec_name: &str) -> Option<Self> {
        let codec_name = CString::new(codec_name).unwrap().into_raw();
        let codec = unsafe { api::_create_video_decoder(codec_name) };
        drop(unsafe { CString::from_raw(codec_name) });

        if !codec.is_null() {
            Some(Self { codec })
        } else {
            None
        }
    }

    pub fn push_packet(&self, pkt: &[u8]) -> bool {
        unsafe { api::_video_decoder_send_packet(self.codec, pkt.as_ptr(), pkt.len()) }
    }

    pub fn read_frame(&self) -> Option<VideoFrame> {
        let mut height = 0;
        let frame = unsafe { api::_video_decoder_read_frame(self.codec, null_mut(), &mut height) };
        if !frame.is_null() {
            Some(VideoFrame::from_raw(frame, height))
        } else {
            None
        }
    }
}

impl Drop for VideoFrameReceiverProcesser {
    fn drop(&mut self) {
        unsafe { api::_release_video_decoder(self.codec) }
    }
}
