use std::{ffi::c_char, os::raw::c_void};

use common::{frame::AudioFrame, strings::Strings};

use crate::{Error, RawPacket};

extern "C" {
    fn codec_create_audio_decoder(codec_name: *const c_char) -> *const c_void;
    fn codec_audio_decoder_send_packet(codec: *const c_void, packet: *const RawPacket) -> bool;
    fn codec_audio_decoder_read_frame(codec: *const c_void) -> *const AudioFrame;
    fn codec_release_audio_decoder(codec: *const c_void);
}

pub struct AudioDecoder(*const c_void);

unsafe impl Send for AudioDecoder {}
unsafe impl Sync for AudioDecoder {}

impl AudioDecoder {
    /// Initialize the AVCodecContext to use the given AVCodec.
    pub fn new(codec: &str) -> Result<Self, Error> {
        log::info!("create AudioDecoder: codec name={:?}", codec);

        let codec = unsafe { codec_create_audio_decoder(Strings::from(codec).as_ptr()) };
        if !codec.is_null() {
            Ok(Self(codec))
        } else {
            Err(Error::AudioDecoder)
        }
    }

    /// Supply raw packet data as input to a decoder.
    pub fn decode(&mut self, data: &[u8], flags: i32, timestamp: u64) -> bool {
        unsafe {
            codec_audio_decoder_send_packet(
                self.0,
                &RawPacket {
                    buffer: data.as_ptr(),
                    len: data.len(),
                    timestamp,
                    flags,
                },
            )
        }
    }

    /// Return decoded output data from a decoder or encoder (when the
    /// AV_CODEC_FLAG_RECON_FRAME flag is used).
    pub fn read(&mut self) -> Option<&AudioFrame> {
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
