#![cfg(any(target_os = "windows", target_os = "linux"))]

mod audio;
mod video;

use std::ffi::c_int;

pub use audio::{AudioDecoder, AudioEncodePacket, AudioEncoder, AudioEncoderSettings};
pub use video::{VideoDecoder, VideoEncodePacket, VideoEncoder, VideoEncoderSettings};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    AudioEncoder,
    VideoEncoder,
    AudioDecoder,
    VideoDecoder,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::AudioDecoder => "failed to create audio decoder",
                Self::AudioEncoder => "failed to create audio encoder",
                Self::VideoDecoder => "failed to create video decoder",
                Self::VideoEncoder => "failed to create video encoder",
            }
        )
    }
}

#[repr(C)]
pub struct RawEncodePacket {
    pub buffer: *const u8,
    pub len: usize,
    pub flags: c_int,
}
