mod audio;
mod util;
mod video;

use std::ffi::{c_char, c_int, c_void};
use std::fmt;

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
use utils::strings::Strings;

pub fn is_hardware_encoder(kind: VideoEncoderType) -> bool {
    match kind {
        VideoEncoderType::Qsv => true,
        VideoEncoderType::Cuda => true,
        _ => false,
    }
}

#[derive(Debug)]
pub enum CodecError {
    NotSupportCodec,
}

impl std::error::Error for CodecError {}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::NotSupportCodec => "NotSupportCodec",
            }
        )
    }
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
    // Write formatted data from variable argument list to sized buffer
    // Composes a string with the same text that would be printed if format was used
    // on printf, but using the elements in the variable argument list identified by
    // arg instead of additional function arguments and storing the resulting
    // content as a C string in the buffer pointed by s (taking n as the maximum
    // buffer capacity to fill).
    //
    // If the resulting string would be longer than n-1 characters, the remaining
    // characters are discarded and not stored, but counted for the value returned
    // by the function.
    //
    // Internally, the function retrieves arguments from the list identified by arg
    // as if va_arg was used on it, and thus the state of arg is likely to be
    // altered by the call.
    //
    // In any case, arg should have been initialized by va_start at some point
    // before the call, and it is expected to be released by va_end at some point
    // after the call.
    fn vsnprintf(s: *mut c_char, n: usize, format: *const c_char, args: va_list) -> c_int;
}

unsafe extern "C" fn logger_proc(
    _: *mut c_void,
    level: c_int,
    message: *const c_char,
    args: va_list,
) {
    let mut chars: [c_char; 1024] = [0; 1024];
    vsnprintf(chars.as_mut_ptr(), 2048, message, args);

    let level: LoggerLevel = std::mem::transmute(level);
    if let Ok(message) = Strings::from(chars.as_ptr()).to_string() {
        log::log!(
            target: "ffmpeg",
            level.into(),
            "{}",
            message.as_str().strip_suffix("\n").unwrap_or(&message)
        );
    }
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
