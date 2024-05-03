use std::{ffi::c_char, os::raw::c_void};

use common::{frame::AudioFrame, strings::Strings};

use crate::Error;

extern "C" {
    fn codec_create_audio_decoder(codec_name: *const c_char) -> *const c_void;
    fn codec_audio_decoder_send_packet(codec: *const c_void, buf: *const u8, size: usize) -> bool;
    fn codec_audio_decoder_read_frame(codec: *const c_void) -> *const AudioFrame;
    fn codec_release_audio_decoder(codec: *const c_void);
}

pub struct AudioDecoder(*const c_void);

unsafe impl Send for AudioDecoder {}
unsafe impl Sync for AudioDecoder {}

impl AudioDecoder {
    pub fn new(codec_name: &str) -> Result<Self, Error> {
        log::info!("create AudioDecoder: codec name={:?}", codec_name);

        let codec = unsafe { codec_create_audio_decoder(Strings::from(codec_name).as_ptr()) };
        if !codec.is_null() {
            Ok(Self(codec))
        } else {
            Err(Error::AudioDecoder)
        }
    }

    pub fn decode(&self, pkt: &[u8]) -> bool {
        unsafe { codec_audio_decoder_send_packet(self.0, pkt.as_ptr(), pkt.len()) }
    }

    pub fn read(&self) -> Option<&AudioFrame> {
        let frame = unsafe { codec_audio_decoder_read_frame(self.0) };
        if !frame.is_null() {
            Some(unsafe { &*frame })
        } else {
            None
        }
    }
}

impl Drop for AudioDecoder {
    fn drop(&mut self) {
        log::info!("close AudioDecoder");

        unsafe { codec_release_audio_decoder(self.0) }
    }
}
