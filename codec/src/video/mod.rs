mod decode;
mod encode;

use std::ffi::c_char;

use common::strings::Strings;
pub use decode::VideoDecoder;
pub use encode::{VideoEncodePacket, VideoEncoder, VideoEncoderSettings};

extern "C" {
    pub fn codec_find_video_encoder() -> *const c_char;
    pub fn codec_find_video_decoder() -> *const c_char;
}

/// Automatically search for encoders, limited hardware, fallback to software
/// implementation if hardware acceleration unit is not found.
pub fn find_video_encoder() -> String {
    Strings::from(unsafe { codec_find_video_encoder() })
        .to_string()
        .unwrap()
}

/// Automatically search for decoders, limited hardware, fallback to software
/// implementation if hardware acceleration unit is not found.
pub fn find_video_decoder() -> String {
    Strings::from(unsafe { codec_find_video_decoder() })
        .to_string()
        .unwrap()
}
