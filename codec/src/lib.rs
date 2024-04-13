pub mod audio;
pub mod video;

#[allow(unused_imports)]
use std::ffi::{c_char, CString};

#[repr(i32)]
#[derive(Clone, Copy)]
pub enum BufferFlag {
    KeyFrame = 1,
    Config = 2,
    EndOfStream = 4,
    Partial = 8,
}

#[cfg(feature = "frame")]
mod api {
    use std::ffi::{c_char, c_int, c_void};

    use crate::free_cstring;

    pub type VideoEncoder = *const c_void;

    #[repr(C)]
    pub struct VideoEncoderSettings {
        pub codec_name: *const c_char,
        pub max_b_frames: u8,
        pub frame_rate: u8,
        pub width: u32,
        pub height: u32,
        pub bit_rate: u64,
        pub key_frame_interval: u32,
    }

    impl Drop for VideoEncoderSettings {
        fn drop(&mut self) {
            free_cstring(self.codec_name);
        }
    }

    #[repr(C)]
    pub struct VideoFrame {
        pub buffer: [*const u8; 4],
        pub stride: [c_int; 4],
    }

    #[repr(C)]
    pub struct VideoEncodePacket {
        pub buffer: *const u8,
        pub len: usize,
        pub flags: c_int,
    }

    extern "C" {
        pub fn _create_video_encoder(settings: *const VideoEncoderSettings) -> VideoEncoder;
        pub fn _video_encoder_send_frame(codec: VideoEncoder, frame: *const VideoFrame) -> c_int;
        pub fn _video_encoder_read_packet(codec: VideoEncoder) -> *const VideoEncodePacket;
        pub fn _unref_video_encoder_packet(codec: VideoEncoder);
        pub fn _release_video_encoder(codec: VideoEncoder);
    }
}

#[cfg(feature = "frame")]
pub(crate) fn to_c_str(str: &str) -> *const c_char {
    CString::new(str).unwrap().into_raw()
}

#[cfg(feature = "frame")]
pub(crate) fn free_cstring(str: *const c_char) {
    if !str.is_null() {
        drop(unsafe { CString::from_raw(str as *mut c_char) })
    }
}
