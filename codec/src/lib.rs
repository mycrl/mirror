mod audio;
mod video;

use std::ffi::c_int;

pub use audio::{AudioEncodePacket, AudioEncoder, AudioEncoderSettings};
pub use video::{VideoDecoder, VideoEncodePacket, VideoEncoder, VideoEncoderSettings};

#[repr(C)]
pub struct RawEncodePacket {
    pub buffer: *const u8,
    pub len: usize,
    pub flags: c_int,
}
