use std::env;

use anyhow::{Result, bail};

pub enum Command {
    Run,
    InsertDemoCritical,
    Help,
}

impl Command {
    pub fn parse() -> Result<Self> {
        match env::args().nth(1).as_deref() {
            None | Some("run") => Ok(Self::Run),
            Some("insert-demo-critical") => Ok(Self::InsertDemoCritical),
            Some("--help") | Some("-h") | Some("help") => Ok(Self::Help),
            Some(other) => bail!("unknown command: {other}"),
        }
    }
}

pub fn print_help() {
    println!(
        "rust-edge

Commands:
  cargo run
  cargo run -- run
  cargo run -- insert-demo-critical
  cargo run -- --help

Environment:
  POWERSYNC_URL
  POWERSYNC_TOKEN
  DEVICE_ID
  FIELDMIND_DB_PATH
  BACKEND_WRITE_URL
  BACKEND_WRITE_TOKEN
  POWERSYNC_STREAM
  POWERSYNC_STREAM_PARAMS"
    );
}
