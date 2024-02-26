#![cfg(not(feature = "android"))]

use crate::{
    api::{
        create_video_encoder, release_video_encoder, release_video_encoder_packet,
        video_encoder_read_packet, video_encoder_send_frame,
    },
    free_cstring, to_c_str,
};

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
    pub key_frame: bool,
    pub buffer: &'a [u8],
    pub stride_y: u32,
    pub stride_uv: u32,
}

impl<'a> VideoFrame<'a> {
    fn as_raw(&self) -> crate::api::VideoFrame {
        crate::api::VideoFrame {
            key_frame: self.key_frame,
            buffer: self.buffer.as_ptr(),
            len: self.buffer.len(),
            stride_y: self.stride_y,
            stride_uv: self.stride_uv,
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
        unsafe { release_video_encoder_packet(self.codec) }
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
        let codec = unsafe { create_video_encoder(&settings) };
        free_cstring(settings.codec_name);

        if !codec.is_null() {
            Some(Self { codec })
        } else {
            None
        }
    }

    pub fn encode(&self, frame: &VideoFrame) -> Vec<Vec<u8>> {
        if unsafe { video_encoder_send_frame(self.codec, &frame.as_raw()) } != 0 {
            return Vec::new();
        }

        let mut ret = Vec::with_capacity(10);
        loop {
            let packet = unsafe { video_encoder_read_packet(self.codec) };
            if !packet.is_null() {
                let pkt = VideoEncodePacket::from_raw(self.codec, packet);
                ret.push(pkt.buffer.to_vec())
            } else {
                break;
            }
        }

        ret
    }
}

impl Drop for VideoFrameSenderProcesser {
    fn drop(&mut self) {
        unsafe { release_video_encoder(self.codec) }
    }
}
