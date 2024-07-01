use std::{
    fmt::Debug,
    fs::{File, OpenOptions},
    io::Write,
};

use log::LevelFilter;
use simplelog::{
    ColorChoice, CombinedLogger, ConfigBuilder, TermLogger, TerminalMode, WriteLogger,
};

pub fn init(name: &str, level: LevelFilter) -> anyhow::Result<()> {
    let config = ConfigBuilder::new()
        .set_time_offset_to_local()
        .unwrap()
        .set_location_level(level)
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
                .append(false)
                .truncate(true)
                .open(name)?,
        ),
    ])?;

    Ok(())
}

pub struct FormatLogger(File);

impl FormatLogger {
    pub fn new(name: &str) -> anyhow::Result<Self> {
        Ok(Self(
            OpenOptions::new()
                .create(true)
                .write(true)
                .append(false)
                .truncate(true)
                .open(name)?,
        ))
    }

    pub fn log<T: Debug>(&mut self, message: &T) -> anyhow::Result<()> {
        self.0.write_all(format!("{:?}\r\n", message).as_bytes())?;
        Ok(())
    }
}
