use std::{ffi::c_char, os::raw::c_void};

use common::{frame::VideoFrame, strings::Strings};

extern "C" {
    fn _create_video_decoder(codec_name: *const c_char) -> *const c_void;
    fn _video_decoder_send_packet(codec: *const c_void, buf: *const u8, size: usize) -> bool;
    fn _video_decoder_read_frame(codec: *const c_void) -> *const VideoFrame;
    fn _release_video_decoder(codec: *const c_void);
}

pub struct VideoDecoder {
    codec: *const c_void,
}

unsafe impl Send for VideoDecoder {}
unsafe impl Sync for VideoDecoder {}

impl VideoDecoder {
    pub fn new(codec_name: &str) -> Option<Self> {
        log::info!("create VideoDecoder: codec name={:?}", codec_name);

        let codec = unsafe { _create_video_decoder(Strings::from(codec_name).as_ptr()) };
        if !codec.is_null() {
            Some(Self { codec })
        } else {
            log::error!("Failed to create VideoDecoder");

            None
        }
    }

    pub fn decode(&self, pkt: &[u8]) -> bool {
        unsafe { _video_decoder_send_packet(self.codec, pkt.as_ptr(), pkt.len()) }
    }

    pub fn read(&self) -> Option<&VideoFrame> {
        let frame = unsafe { _video_decoder_read_frame(self.codec) };
        if !frame.is_null() {
            Some(unsafe { &*frame })
        } else {
            None
        }
    }
}

impl Drop for VideoDecoder {
    fn drop(&mut self) {
        log::info!("close VideoDecoder");

        unsafe { _release_video_decoder(self.codec) }
    }
}
