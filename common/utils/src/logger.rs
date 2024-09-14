use std::fs::OpenOptions;

use log::LevelFilter;
use simplelog::{
    format_description, ColorChoice, CombinedLogger, ConfigBuilder, SharedLogger, TermLogger,
    TerminalMode, WriteLogger,
};

/// Initialize logging
///
/// The `name` argument is the name of the log file. This will create a log file
/// in the current directory and output to stdio at the same time.
pub fn init(level: LevelFilter, filename: Option<&str>) -> anyhow::Result<()> {
    let config = ConfigBuilder::new()
        .set_time_format_custom(format_description!(
            "[month]-[day] [hour]:[minute]:[second]"
        ))
        .set_thread_level(LevelFilter::Error)
        .set_location_level(LevelFilter::Error)
        .build();

    let mut loggers: Vec<Box<dyn SharedLogger>> = vec![TermLogger::new(
        level,
        config.clone(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )];

    if let Some(filename) = filename {
        loggers.push(WriteLogger::new(
            level,
            config,
            OpenOptions::new()
                .create(true)
                .write(true)
                .append(true)
                .open(filename)?,
        ));
    }

    CombinedLogger::init(loggers)?;
    Ok(())
}
