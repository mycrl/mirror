use std::ffi::{c_char, c_int, c_void, CString};

use common::frame::VideoFrame;

extern "C" {
    fn codec_create_video_encoder(settings: *const RawVideoEncoderSettings) -> *const c_void;
    fn codec_video_encoder_send_frame(codec: *const c_void, frame: *const VideoFrame) -> bool;
    fn codec_video_encoder_read_packet(codec: *const c_void) -> *const RawVideoEncodePacket;
    fn codec_unref_video_encoder_packet(codec: *const c_void);
    fn codec_release_video_encoder(codec: *const c_void);
}

#[repr(C)]
pub struct RawVideoEncoderSettings {
    pub codec_name: *const c_char,
    pub max_b_frames: u8,
    pub frame_rate: u8,
    pub width: u32,
    pub height: u32,
    pub bit_rate: u64,
    pub key_frame_interval: u32,
}

impl Drop for RawVideoEncoderSettings {
    fn drop(&mut self) {
        drop(unsafe { CString::from_raw(self.codec_name as *mut _) })
    }
}

#[repr(C)]
pub struct RawVideoEncodePacket {
    pub buffer: *const u8,
    pub len: usize,
    pub flags: c_int,
}

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
    fn as_raw(&self) -> RawVideoEncoderSettings {
        RawVideoEncoderSettings {
            codec_name: CString::new(self.codec_name.as_str()).unwrap().into_raw(),
            key_frame_interval: self.key_frame_interval,
            max_b_frames: self.max_b_frames,
            frame_rate: self.frame_rate,
            width: self.width,
            height: self.height,
            bit_rate: self.bit_rate,
        }
    }
}

#[repr(C)]
pub struct VideoEncodePacket<'a> {
    codec: *const c_void,
    pub buffer: &'a [u8],
    pub flags: i32,
}

impl Drop for VideoEncodePacket<'_> {
    fn drop(&mut self) {
        unsafe { codec_unref_video_encoder_packet(self.codec) }
    }
}

impl<'a> VideoEncodePacket<'a> {
    fn from_raw(codec: *const c_void, ptr: *const RawVideoEncodePacket) -> Self {
        let raw = unsafe { &*ptr };
        Self {
            buffer: unsafe { std::slice::from_raw_parts(raw.buffer, raw.len) },
            flags: raw.flags,
            codec,
        }
    }
}

pub struct VideoEncoder {
    codec: *const c_void,
}

unsafe impl Send for VideoEncoder {}
unsafe impl Sync for VideoEncoder {}

impl VideoEncoder {
    pub fn new(settings: &VideoEncoderSettings) -> Option<Self> {
        log::info!("create VideoEncoder: settings={:?}", settings);

        let settings = settings.as_raw();
        let codec = unsafe { codec_create_video_encoder(&settings) };
        if !codec.is_null() {
            Some(Self { codec })
        } else {
            log::error!("Failed to create VideoEncoder");

            None
        }
    }

    pub fn encode(&self, frame: &VideoFrame) -> bool {
        unsafe { codec_video_encoder_send_frame(self.codec, frame) }
    }

    pub fn read(&self) -> Option<VideoEncodePacket> {
        let packet = unsafe { codec_video_encoder_read_packet(self.codec) };
        if !packet.is_null() {
            Some(VideoEncodePacket::from_raw(self.codec, packet))
        } else {
            None
        }
    }
}

impl Drop for VideoEncoder {
    fn drop(&mut self) {
        log::info!("close VideoEncoder");

        unsafe { codec_release_video_encoder(self.codec) }
    }
}
