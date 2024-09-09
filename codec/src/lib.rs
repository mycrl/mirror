mod audio;
mod util;
mod video;

use std::ffi::{c_char, c_int, c_void};

pub use self::{
    audio::{
        create_opus_identification_header, AudioDecoder, AudioDecoderError, AudioEncoder,
        AudioEncoderError, AudioEncoderSettings,
    },
    video::{
        VideoDecoder, VideoDecoderError, VideoDecoderSettings, VideoDecoderType, VideoEncoder,
        VideoEncoderError, VideoEncoderSettings, VideoEncoderType,
    },
};

use ffmpeg_sys_next::*;
use log::Level;

pub fn is_hardware_encoder(kind: VideoEncoderType) -> bool {
    match kind {
        VideoEncoderType::Qsv => true,
        VideoEncoderType::Cuda => true,
        _ => false,
    }
}

#[repr(C)]
#[derive(Debug)]
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

unsafe extern "C" fn logger_proc(_: *mut c_void, level: c_int, message: *const c_char, args: va_list) {

}

pub fn startup() {
    unsafe {
        av_log_set_callback(Some(logger_proc));
    }
}

pub fn shutdown() {
    unsafe {
        av_log_set_callback(None);
    }
}
