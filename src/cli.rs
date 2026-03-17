use std::env;

use anyhow::{Result, bail};

pub enum Command {
    Run,
    Login,
    Logout,
    WhoAmI,
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
            Some("login") | Some("--login") => Ok(Self::Login),
            Some("logout") | Some("--logout") => Ok(Self::Logout),
            Some("whoami") | Some("--whoami") | Some("who") => Ok(Self::WhoAmI),
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
        "fieldmid — FieldMid Edge Daemon

Commands:
  fieldmid                        Start the edge daemon (TUI or headless)
  fieldmid login                  Authenticate as org admin or supervisor
  fieldmid logout                 Clear stored session
  fieldmid whoami                 Show current auth status and org info
  fieldmid check-connectivity     Test PowerSync connectivity
  fieldmid latest-incidents       Show latest critical incidents from local DB
  fieldmid help                   Show this help

Local development:
  cargo run --bin fieldmid
  cargo run --bin fieldmid -- login
  cargo run --bin fieldmid -- whoami
  cargo run --bin fieldmid -- check-connectivity
  cargo run --bin fieldmid -- latest-incidents

Environment:
  SUPABASE_URL          Supabase project URL (for auth)
  SUPABASE_ANON_KEY     Supabase anonymous key (for auth)
  POWERSYNC_URL         PowerSync instance URL
  DEVICE_ID             Edge device identifier
  FIELDMID_DB_PATH      Local SQLite database path"
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
        assert!(matches!(
            Command::parse_arg(Some("login")),
            Ok(Command::Login)
        ));
        assert!(matches!(
            Command::parse_arg(Some("logout")),
            Ok(Command::Logout)
        ));
        assert!(matches!(
            Command::parse_arg(Some("whoami")),
            Ok(Command::WhoAmI)
        ));
    }
}
