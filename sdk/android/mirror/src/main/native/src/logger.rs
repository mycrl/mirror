#![allow(unused)]

use std::ffi::{c_char, c_int};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Verbose = 2,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn from_level(level: log::Level) -> Self {
        match level {
            log::Level::Trace => Self::Verbose,
            log::Level::Debug => Self::Debug,
            log::Level::Info => Self::Info,
            log::Level::Warn => Self::Warn,
            log::Level::Error => Self::Error,
        }
    }
}

#[cfg(target_os = "android")]
extern "C" {
    // __android_log_write
    //
    //
    // int __android_log_write(
    //   int prio,
    //   const char *tag,
    //   const char *text
    // )
    //
    // Writes the constant string text to the log, with priority prio and tag tag.
    fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) -> c_int;
}

pub struct AndroidLogger;

impl AndroidLogger {
    pub fn init() {
        log::set_boxed_logger(Box::new(Self)).unwrap();
        log::set_max_level(log::LevelFilter::Info);
        std::panic::set_hook(Box::new(|info| log::error!("{:?}", info)))
    }
}

impl log::Log for AndroidLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() == log::LevelFilter::Info
    }

    #[allow(unused_variables)]
    fn log(&self, record: &log::Record) {
        #[cfg(target_os = "android")]
        unsafe {
            __android_log_write(
                LogLevel::from_level(record.level()) as c_int,
                "com.github.mycrl.mirror\0".as_ptr() as *const _,
                format!("{}\0", record.args()).as_ptr() as *const _,
            );
        }
    }

    fn flush(&self) {}
}
