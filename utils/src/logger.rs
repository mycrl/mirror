#[cfg(all(not(debug_assertions), target_os = "windows"))]
use std::fs::{create_dir, metadata};

#[cfg(target_os = "android")]
use std::ffi::{c_char, c_int};

use fern::Dispatch;
use log::LevelFilter;
use thiserror::Error;

#[cfg(not(debug_assertions))]
use chrono::Local;

#[cfg(debug_assertions)]
use fern::colors::{Color, ColoredLevelConfig};

#[cfg(all(not(debug_assertions), target_os = "windows"))]
use fern::DateBased;

#[derive(Debug, Error)]
pub enum LoggerInitError {
    #[error(transparent)]
    LogError(#[from] log::SetLoggerError),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

#[allow(unused_variables)]
fn init_logger(level: LevelFilter, path: Option<&str>) -> Result<(), LoggerInitError> {
    let mut logger = Dispatch::new().level(level);
    // .level_for("wgpu", LevelFilter::Warn)
    // .level_for("wgpu_core", LevelFilter::Warn)
    // .level_for("wgpu_hal", LevelFilter::Warn)
    // .level_for("wgpu_hal::auxil::dxgi::exception", LevelFilter::Error);

    #[cfg(debug_assertions)]
    {
        let colors = ColoredLevelConfig::new()
            .info(Color::Blue)
            .warn(Color::Yellow)
            .error(Color::Red);

        logger = logger
            .format(move |out, message, record| {
                out.finish(format_args!(
                    "[{}] - ({}) - {}",
                    colors.color(record.level()),
                    record.file_static().unwrap_or("*"),
                    message
                ))
            })
            .chain(std::io::stdout());
    }

    #[cfg(not(debug_assertions))]
    {
        logger = logger.format(move |out, message, record| {
            out.finish(format_args!(
                "{} - [{}] - ({}) - {}",
                Local::now().format("%m-%d %H:%M:%S"),
                record.level(),
                record.file_static().unwrap_or("*"),
                message
            ))
        });

        if let Some(path) = path {
            if metadata(path).is_err() {
                create_dir(path)?;
            }

            logger = logger.chain(DateBased::new(path, "%Y-%m-%d-mirror.log"));
        }
    }

    logger.apply()?;
    Ok(())
}

pub fn init(level: LevelFilter, path: Option<&str>) -> Result<(), LoggerInitError> {
    init_logger(level, path)
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AndroidLogLevel {
    Verbose = 2,
    Debug,
    Info,
    Warn,
    Error,
}

impl AndroidLogLevel {
    pub fn from_level(level: log::Level) -> Self {
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

        std::panic::set_hook(Box::new(|info| {
            log::error!(
                "pnaic: location={:?}, message={:?}",
                info.location(),
                info.payload().downcast_ref::<String>(),
            );
        }));
    }
}

impl log::Log for AndroidLogger {
    fn flush(&self) {}
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    #[allow(unused_variables)]
    fn log(&self, record: &log::Record) {
        #[cfg(target_os = "android")]
        unsafe {
            __android_log_write(
                AndroidLogLevel::from_level(record.level()) as c_int,
                "com.github.mycrl.mirror\0".as_ptr() as *const _,
                format!("{}\0", record.args()).as_ptr() as *const _,
            );
        }
    }
}