#[cfg(any(target_os = "windows", target_os = "linux"))]

use std::ffi::c_int;

use crate::{to_c_str, api};

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
    fn as_raw(&self) -> crate::api::VideoEncoderSettings {
        crate::api::VideoEncoderSettings {
            codec_name: to_c_str(&self.codec_name),
            max_b_frames: self.max_b_frames,
            frame_rate: self.frame_rate,
            width: self.width,
            height: self.height,
            bit_rate: self.bit_rate,
            key_frame_interval: self.key_frame_interval,
        }
    }
}

pub struct VideoFrame<'a> {
    pub buffer: [&'a [u8]; 4],
    pub stride: [u32; 4],
}

impl<'a> VideoFrame<'a> {
    fn as_raw(&self) -> crate::api::VideoFrame {
        crate::api::VideoFrame {
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
}

#[repr(C)]
pub struct VideoEncodePacket<'a> {
    codec: crate::api::VideoEncoder,
    pub buffer: &'a [u8],
    pub flags: i32,
}

impl Drop for VideoEncodePacket<'_> {
    fn drop(&mut self) {
        unsafe { api::_unref_video_encoder_packet(self.codec) }
    }
}

impl<'a> VideoEncodePacket<'a> {
    fn from_raw(
        codec: crate::api::VideoEncoder,
        ptr: *const crate::api::VideoEncodePacket,
    ) -> Self {
        let raw = unsafe { &*ptr };
        Self {
            buffer: unsafe { std::slice::from_raw_parts(raw.buffer, raw.len) },
            flags: raw.flags,
            codec,
        }
    }
}

pub struct VideoFrameSenderProcesser {
    codec: crate::api::VideoEncoder,
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
        unsafe { api::_video_encoder_send_frame(self.codec, &frame.as_raw()) == 0 }
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
