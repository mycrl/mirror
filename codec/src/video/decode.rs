use std::{ffi::c_char, os::raw::c_void};

use common::{frame::VideoFrame, strings::Strings};

extern "C" {
    fn codec_create_video_decoder(codec_name: *const c_char) -> *const c_void;
    fn codec_video_decoder_send_packet(codec: *const c_void, buf: *const u8, size: usize) -> bool;
    fn codec_video_decoder_read_frame(codec: *const c_void) -> *const VideoFrame;
    fn codec_release_video_decoder(codec: *const c_void);
}

pub struct VideoDecoder(*const c_void);

unsafe impl Send for VideoDecoder {}
unsafe impl Sync for VideoDecoder {}

impl VideoDecoder {
    pub fn new(codec_name: &str) -> Option<Self> {
        log::info!("create VideoDecoder: codec name={:?}", codec_name);

        let codec = unsafe { codec_create_video_decoder(Strings::from(codec_name).as_ptr()) };
        if !codec.is_null() {
            Some(Self(codec))
        } else {
            log::error!("Failed to create VideoDecoder");

            None
        }
    }

    pub fn decode(&self, pkt: &[u8]) -> bool {
        unsafe { codec_video_decoder_send_packet(self.0, pkt.as_ptr(), pkt.len()) }
    }

    pub fn read(&self) -> Option<&VideoFrame> {
        let frame = unsafe { codec_video_decoder_read_frame(self.0) };
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

        unsafe { codec_release_video_decoder(self.0) }
    }
}
