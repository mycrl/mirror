use std::ffi::{c_char, c_void, CString};

use common::frame::VideoFrame;

use crate::{Error, RawEncodePacket};

extern "C" {
    fn codec_create_video_encoder(settings: *const RawVideoEncoderSettings) -> *const c_void;
    fn codec_video_encoder_send_frame(codec: *const c_void, frame: *const VideoFrame) -> bool;
    fn codec_video_encoder_read_packet(codec: *const c_void) -> *const RawEncodePacket;
    fn codec_unref_video_encoder_packet(codec: *const c_void);
    fn codec_release_video_encoder(codec: *const c_void);
}

#[repr(C)]
pub struct RawVideoEncoderSettings {
    pub codec_name: *const c_char,
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

#[derive(Debug, Clone)]
pub struct VideoEncoderSettings {
    /// Name of the codec implementation.
    ///
    /// The name is globally unique among encoders and among decoders (but an
    /// encoder and a decoder can share the same name). This is the primary way
    /// to find a codec from the user perspective.
    pub codec_name: String,
    pub frame_rate: u8,
    /// picture width / height
    pub width: u32,
    /// picture width / height
    pub height: u32,
    /// the average bitrate
    pub bit_rate: u64,
    /// the number of pictures in a group of pictures, or 0 for intra_only
    pub key_frame_interval: u32,
}

impl VideoEncoderSettings {
    fn as_raw(&self) -> RawVideoEncoderSettings {
        RawVideoEncoderSettings {
            codec_name: CString::new(self.codec_name.as_str()).unwrap().into_raw(),
            key_frame_interval: self.key_frame_interval,
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
    fn from_raw(codec: *const c_void, ptr: *const RawEncodePacket) -> Self {
        let raw = unsafe { &*ptr };
        Self {
            buffer: unsafe { std::slice::from_raw_parts(raw.buffer, raw.len) },
            flags: raw.flags,
            codec,
        }
    }
}

pub struct VideoEncoder(*const c_void);

unsafe impl Send for VideoEncoder {}
unsafe impl Sync for VideoEncoder {}

impl VideoEncoder {
    /// Initialize the AVCodecContext to use the given AVCodec.
    pub fn new(settings: &VideoEncoderSettings) -> Result<Self, Error> {
        log::info!("create VideoEncoder: settings={:?}", settings);

        let settings = settings.as_raw();
        let codec = unsafe { codec_create_video_encoder(&settings) };
        if !codec.is_null() {
            Ok(Self(codec))
        } else {
            Err(Error::VideoEncoder)
        }
    }

    /// Supply a raw video or audio frame to the encoder.
    pub fn encode(&self, frame: &VideoFrame) -> bool {
        unsafe { codec_video_encoder_send_frame(self.0, frame) }
    }

    /// Read encoded data from the encoder.
    pub fn read(&self) -> Option<VideoEncodePacket> {
        let packet = unsafe { codec_video_encoder_read_packet(self.0) };
        if !packet.is_null() {
            Some(VideoEncodePacket::from_raw(self.0, packet))
        } else {
            None
        }
    }
}

impl Drop for VideoEncoder {
    fn drop(&mut self) {
        log::info!("close VideoEncoder");

        unsafe { codec_release_video_encoder(self.0) }
    }
}
