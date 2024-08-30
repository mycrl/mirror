use crate::{Error, RawPacket};

#[allow(unused)]
use std::{
    ffi::{c_char, CString},
    os::raw::c_void,
    ptr::null_mut,
};

use frame::VideoFrame;
use utils::strings::Strings;

#[cfg(target_os = "windows")]
use utils::win32::{Direct3DDevice, Interface};

extern "C" {
    pub fn codec_find_video_encoder() -> *const c_char;
    pub fn codec_find_video_decoder() -> *const c_char;
    fn codec_create_video_encoder(settings: *const RawVideoEncoderSettings) -> *const c_void;
    fn codec_video_encoder_copy_frame(codec: *const c_void, frame: *const VideoFrame) -> bool;
    fn codec_video_encoder_send_frame(codec: *const c_void) -> bool;
    fn codec_video_encoder_read_packet(codec: *const c_void) -> *const RawPacket;
    fn codec_unref_video_encoder_packet(codec: *const c_void);
    fn codec_release_video_encoder(codec: *const c_void);
    fn codec_create_video_decoder(settings: *const RawVideoDecoderSettings) -> *const c_void;
    fn codec_video_decoder_send_packet(codec: *const c_void, packet: RawPacket) -> bool;
    fn codec_video_decoder_read_frame(codec: *const c_void) -> *const VideoFrame;
    fn codec_release_video_decoder(codec: *const c_void);
}

#[repr(C)]
pub struct RawVideoEncoderSettings {
    #[cfg(target_os = "windows")]
    pub d3d11_device: *const c_void,
    #[cfg(target_os = "windows")]
    pub d3d11_device_context: *const c_void,
    pub codec: *const c_char,
    pub frame_rate: u8,
    pub width: u32,
    pub height: u32,
    pub bit_rate: u64,
    pub key_frame_interval: u32,
}

impl Drop for RawVideoEncoderSettings {
    fn drop(&mut self) {
        drop(unsafe { CString::from_raw(self.codec as *mut _) })
    }
}

#[derive(Debug, Clone)]
pub struct VideoEncoderSettings {
    /// Name of the codec implementation.
    ///
    /// The name is globally unique among encoders and among decoders (but an
    /// encoder and a decoder can share the same name). This is the primary way
    /// to find a codec from the user perspective.
    pub codec: String,
    pub frame_rate: u8,
    /// picture width / height
    pub width: u32,
    /// picture width / height
    pub height: u32,
    /// the average bitrate
    pub bit_rate: u64,
    /// the number of pictures in a group of pictures, or 0 for intra_only
    pub key_frame_interval: u32,
    #[cfg(target_os = "windows")]
    pub direct3d: Option<Direct3DDevice>,
}

impl VideoEncoderSettings {
    fn as_raw(&self) -> RawVideoEncoderSettings {
        RawVideoEncoderSettings {
            codec: CString::new(self.codec.as_str()).unwrap().into_raw(),
            key_frame_interval: self.key_frame_interval,
            frame_rate: self.frame_rate,
            width: self.width,
            height: self.height,
            bit_rate: self.bit_rate,

            #[cfg(target_os = "windows")]
            d3d11_device: self
                .direct3d
                .as_ref()
                .map(|it| it.device.as_raw())
                .unwrap_or_else(|| null_mut()),

            #[cfg(target_os = "windows")]
            d3d11_device_context: self
                .direct3d
                .as_ref()
                .map(|it| it.context.as_raw())
                .unwrap_or_else(|| null_mut()),
        }
    }
}

#[repr(C)]
pub struct RawVideoDecoderSettings {
    #[cfg(target_os = "windows")]
    pub d3d11_device: *const c_void,
    #[cfg(target_os = "windows")]
    pub d3d11_device_context: *const c_void,
    pub codec: *const c_char,
}

impl Drop for RawVideoDecoderSettings {
    fn drop(&mut self) {
        drop(unsafe { CString::from_raw(self.codec as *mut _) })
    }
}

#[derive(Debug, Clone)]
pub struct VideoDecoderSettings {
    /// Name of the codec implementation.
    ///
    /// The name is globally unique among encoders and among decoders (but an
    /// encoder and a decoder can share the same name). This is the primary way
    /// to find a codec from the user perspective.
    pub codec: String,
    #[cfg(target_os = "windows")]
    pub direct3d: Option<Direct3DDevice>,
}

impl VideoDecoderSettings {
    fn as_raw(&self) -> RawVideoDecoderSettings {
        RawVideoDecoderSettings {
            codec: CString::new(self.codec.as_str()).unwrap().into_raw(),

            #[cfg(target_os = "windows")]
            d3d11_device: self
                .direct3d
                .as_ref()
                .map(|it| it.device.as_raw())
                .unwrap_or_else(|| null_mut()),

            #[cfg(target_os = "windows")]
            d3d11_device_context: self
                .direct3d
                .as_ref()
                .map(|it| it.context.as_raw())
                .unwrap_or_else(|| null_mut()),
        }
    }
}

#[repr(C)]
pub struct VideoEncodePacket<'a> {
    codec: *const c_void,
    pub buffer: &'a [u8],
    pub flags: i32,
    pub timestamp: u64,
}

impl Drop for VideoEncodePacket<'_> {
    fn drop(&mut self) {
        unsafe { codec_unref_video_encoder_packet(self.codec) }
    }
}

impl<'a> VideoEncodePacket<'a> {
    fn from_raw(codec: *const c_void, ptr: *const RawPacket) -> Self {
        let raw = unsafe { &*ptr };
        Self {
            buffer: unsafe { std::slice::from_raw_parts(raw.buffer, raw.len) },
            timestamp: raw.timestamp,
            flags: raw.flags,
            codec,
        }
    }
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

pub struct VideoEncoder(*const c_void);

unsafe impl Send for VideoEncoder {}
unsafe impl Sync for VideoEncoder {}

impl VideoEncoder {
    /// Initialize the AVCodecContext to use the given AVCodec.
    pub fn new(settings: &VideoEncoderSettings) -> Result<Self, Error> {
        log::info!("create VideoEncoder: settings={:?}", settings);

        let settings = settings.as_raw();
        let codec = unsafe { codec_create_video_encoder(&settings) };
        if !codec.is_null() {
            Ok(Self(codec))
        } else {
            Err(Error::VideoEncoder)
        }
    }

    pub fn send_frame(&mut self, frame: &VideoFrame) -> bool {
        unsafe { codec_video_encoder_copy_frame(self.0, frame) }
    }

    /// Supply a raw video or audio frame to the encoder.
    pub fn encode(&mut self) -> bool {
        unsafe { codec_video_encoder_send_frame(self.0) }
    }

    /// Read encoded data from the encoder.
    pub fn read(&mut self) -> Option<VideoEncodePacket> {
        let packet = unsafe { codec_video_encoder_read_packet(self.0) };
        if !packet.is_null() {
            Some(VideoEncodePacket::from_raw(self.0, packet))
        } else {
            None
        }
    }
}

impl Drop for VideoEncoder {
    fn drop(&mut self) {
        log::info!("close VideoEncoder");

        unsafe { codec_release_video_encoder(self.0) }
    }
}

pub struct VideoDecoder(*const c_void);

unsafe impl Send for VideoDecoder {}
unsafe impl Sync for VideoDecoder {}

impl VideoDecoder {
    /// Initialize the AVCodecContext to use the given AVCodec.
    pub fn new(settings: &VideoDecoderSettings) -> Result<Self, Error> {
        log::info!("create VideoDecoder: settings={:?}", settings);

        let settings = settings.as_raw();
        let codec = unsafe { codec_create_video_decoder(&settings) };
        if !codec.is_null() {
            Ok(Self(codec))
        } else {
            Err(Error::VideoDecoder)
        }
    }

    /// Supply raw packet data as input to a decoder.
    pub fn decode(&mut self, data: &[u8], flags: i32, timestamp: u64) -> bool {
        unsafe {
            codec_video_decoder_send_packet(
                self.0,
                RawPacket {
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
    pub fn read(&mut self) -> Option<&VideoFrame> {
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
