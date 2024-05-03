use std::ffi::{c_char, c_void, CString};

use common::frame::AudioFrame;

use crate::RawEncodePacket;

extern "C" {
    fn codec_create_audio_encoder(settings: *const RawAudioEncoderSettings) -> *const c_void;
    fn codec_audio_encoder_send_frame(codec: *const c_void, frame: *const AudioFrame) -> bool;
    fn codec_audio_encoder_read_packet(codec: *const c_void) -> *const RawEncodePacket;
    fn codec_unref_audio_encoder_packet(codec: *const c_void);
    fn codec_release_audio_encoder(codec: *const c_void);
}

#[repr(C)]
pub struct RawAudioEncoderSettings {
    pub codec_name: *const c_char,
    pub bit_rate: u64,
    pub sample_rate: u64,
}

impl Drop for RawAudioEncoderSettings {
    fn drop(&mut self) {
        drop(unsafe { CString::from_raw(self.codec_name as *mut _) })
    }
}

#[derive(Debug, Clone)]
pub struct AudioEncoderSettings {
    pub codec_name: String,
    pub bit_rate: u64,
    pub sample_rate: u64,
}

impl AudioEncoderSettings {
    fn as_raw(&self) -> RawAudioEncoderSettings {
        RawAudioEncoderSettings {
            codec_name: CString::new(self.codec_name.as_str()).unwrap().into_raw(),
            sample_rate: self.sample_rate,
            bit_rate: self.bit_rate,
        }
    }
}

#[repr(C)]
pub struct AudioEncodePacket<'a> {
    codec: *const c_void,
    pub buffer: &'a [u8],
    pub flags: i32,
}

impl Drop for AudioEncodePacket<'_> {
    fn drop(&mut self) {
        unsafe { codec_unref_audio_encoder_packet(self.codec) }
    }
}

impl<'a> AudioEncodePacket<'a> {
    fn from_raw(codec: *const c_void, ptr: *const RawEncodePacket) -> Self {
        let raw = unsafe { &*ptr };
        Self {
            buffer: unsafe { std::slice::from_raw_parts(raw.buffer, raw.len) },
            flags: raw.flags,
            codec,
        }
    }
}

pub struct AudioEncoder(*const c_void);

unsafe impl Send for AudioEncoder {}
unsafe impl Sync for AudioEncoder {}

impl AudioEncoder {
    pub fn new(settings: &AudioEncoderSettings) -> Option<Self> {
        log::info!("create AudioEncoder: settings={:?}", settings);

        let settings = settings.as_raw();
        let codec = unsafe { codec_create_audio_encoder(&settings) };
        if !codec.is_null() {
            Some(Self(codec))
        } else {
            log::error!("Failed to create AudioEncoder");

            None
        }
    }

    pub fn encode(&self, frame: &AudioFrame) -> bool {
        unsafe { codec_audio_encoder_send_frame(self.0, frame) }
    }

    pub fn read(&self) -> Option<AudioEncodePacket> {
        let packet = unsafe { codec_audio_encoder_read_packet(self.0) };
        if !packet.is_null() {
            Some(AudioEncodePacket::from_raw(self.0, packet))
        } else {
            None
        }
    }
}

impl Drop for AudioEncoder {
    fn drop(&mut self) {
        log::info!("close AudioEncoder");

        unsafe { codec_release_audio_encoder(self.0) }
    }
}
