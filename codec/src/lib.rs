pub mod audio;
pub mod video;

use std::ffi::{c_char, c_int};

use common::strings::Strings;
use log::{log, Level};

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
pub struct RawPacket {
    pub buffer: *const u8,
    pub len: usize,
    pub flags: c_int,
    pub timestamp: u64,
}

#[repr(C)]
#[derive(Debug)]
#[allow(dead_code)]
enum LoggerLevel {
    Panic = 0,
    Fatal = 8,
    Error = 16,
    Warn = 24,
    Info = 32,
    Verbose = 40,
    Debug = 48,
    Trace = 56,
}

impl Into<Level> for LoggerLevel {
    fn into(self) -> Level {
        match self {
            Self::Panic | Self::Fatal | Self::Error => Level::Error,
            Self::Info | Self::Verbose => Level::Info,
            Self::Warn => Level::Warn,
            Self::Debug => Level::Debug,
            Self::Trace => Level::Trace,
        }
    }
}

extern "C" {
    fn codec_remove_logger();
    fn codec_set_logger(logger: extern "C" fn(level: LoggerLevel, message: *const c_char));
}

extern "C" fn logger_proc(level: LoggerLevel, message: *const c_char) {
    if let Ok(message) = Strings::from(message).to_string() {
        log!(
            target: "ffmpeg",
            level.into(),
            "{}",
            message.as_str().strip_suffix("\n").unwrap_or(&message)
        );
    }
}

pub fn init() {
    unsafe { codec_set_logger(logger_proc) }
}

pub fn quit() {
    unsafe { codec_remove_logger() }
}
