use std::fs::OpenOptions;

use log::LevelFilter;
use simplelog::{
    format_description, ColorChoice, CombinedLogger, ConfigBuilder, TermLogger, TerminalMode,
    WriteLogger,
};

/// Initialize logging
///
/// The `name` argument is the name of the log file. This will create a log file
/// in the current directory and output to stdio at the same time.
pub fn init(name: &str, level: LevelFilter) -> anyhow::Result<()> {
    let config = ConfigBuilder::new()
        .set_time_format_custom(format_description!(
            "[month]-[day] [hour]:[minute]:[second]"
        ))
        .set_thread_level(LevelFilter::Error)
        .set_location_level(LevelFilter::Error)
        .build();

    CombinedLogger::init(vec![
        TermLogger::new(
            level,
            config.clone(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            level,
            config,
            OpenOptions::new()
                .create(true)
                .write(true)
                .append(true)
                .open(name)?,
        ),
    ])?;

    Ok(())
}
