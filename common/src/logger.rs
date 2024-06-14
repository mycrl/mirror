use std::{
    fmt::Debug,
    fs::{File, OpenOptions},
    io::Write,
};

use log::LevelFilter;
use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, TerminalMode, WriteLogger};

pub fn init(name: &str, level: LevelFilter) -> anyhow::Result<()> {
    CombinedLogger::init(vec![
        TermLogger::new(
            level,
            Config::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            level,
            Config::default(),
            OpenOptions::new()
                .create(true)
                .write(true)
                .append(false)
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
                .open(name)?,
        ))
    }

    pub fn log<T: Debug>(&mut self, message: &T) -> anyhow::Result<()> {
        self.0.write_all(format!("{:?}\r\n", message).as_bytes())?;
        Ok(())
    }
}
