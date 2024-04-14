pub mod audio;
pub mod video;

pub type RawVideoFrame = api::VideoFrame;

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
    use std::ffi::{c_char, c_int, c_void, CString};

    pub type VideoEncoder = *const c_void;
    pub type VideoDecoder = *const c_void;

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
            drop(unsafe { CString::from_raw(self.codec_name as *mut _) })
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
        pub fn _video_encoder_send_frame(codec: VideoEncoder, frame: *const VideoFrame) -> bool;
        pub fn _video_encoder_read_packet(codec: VideoEncoder) -> *const VideoEncodePacket;
        pub fn _unref_video_encoder_packet(codec: VideoEncoder);
        pub fn _release_video_encoder(codec: VideoEncoder);
        pub fn _create_video_decoder(codec_name: *const c_char) -> VideoDecoder;
        pub fn _video_decoder_send_packet(codec: VideoDecoder, buf: *const u8, size: usize)
            -> bool;
        pub fn _video_decoder_read_frame(
            codec: VideoDecoder,
            width: *mut u32,
            height: *mut u32,
        ) -> *const VideoFrame;
        pub fn _release_video_decoder(codec: VideoDecoder);
    }
}
