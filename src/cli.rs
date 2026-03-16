use std::env;

use anyhow::{Result, bail};

pub enum Command {
    Run,
    CheckConnectivity,
    LatestIncidents,
    Help,
}

impl Command {
    pub fn parse() -> Result<Self> {
        Self::parse_arg(env::args().nth(1).as_deref())
    }

    fn parse_arg(arg: Option<&str>) -> Result<Self> {
        match arg {
            None | Some("run") | Some("--run") => Ok(Self::Run),
            Some("check-connectivity") | Some("--check-connectivity") => {
                Ok(Self::CheckConnectivity)
            }
            Some("latest-incidents") | Some("--latest-incidents") => Ok(Self::LatestIncidents),
            Some("--help") | Some("-h") | Some("help") => Ok(Self::Help),
            Some(other) => bail!("unknown command: {other}"),
        }
    }
}

pub fn print_help() {
    println!(
        "fieldmid

Commands:
  fieldmid
  fieldmid --check-connectivity
  fieldmid --latest-incidents
  fieldmid --help

Local development:
  cargo run --bin fieldmid
  cargo run --bin fieldmid -- --check-connectivity
  cargo run --bin fieldmid -- --latest-incidents
  cargo run --bin fieldmid -- --help

Environment:
  POWERSYNC_URL
  POWERSYNC_TOKEN
  DEVICE_ID
  FIELDMID_DB_PATH
  POWERSYNC_STREAM
  POWERSYNC_STREAM_PARAMS"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_defaults_to_run() {
        assert!(matches!(Command::parse_arg(None), Ok(Command::Run)));
    }

    #[test]
    fn parse_known_commands() {
        assert!(matches!(
            Command::parse_arg(Some("check-connectivity")),
            Ok(Command::CheckConnectivity)
        ));
        assert!(matches!(
            Command::parse_arg(Some("--check-connectivity")),
            Ok(Command::CheckConnectivity)
        ));
        assert!(matches!(
            Command::parse_arg(Some("latest-incidents")),
            Ok(Command::LatestIncidents)
        ));
        assert!(matches!(
            Command::parse_arg(Some("--latest-incidents")),
            Ok(Command::LatestIncidents)
        ));
        assert!(matches!(
            Command::parse_arg(Some("--help")),
            Ok(Command::Help)
        ));
    }
}
