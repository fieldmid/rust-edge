use std::env;

use anyhow::{Result, bail};

pub enum Command {
    Run,
    Login,
    Logout,
    WhoAmI,
    Users,
    Requests,
    Doctor,
    CheckConnectivity,
    LatestIncidents,
    InstallHint,
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
            Some("users") | Some("--users") => Ok(Self::Users),
            Some("requests") | Some("--requests") | Some("join-requests") => Ok(Self::Requests),
            Some("doctor") | Some("--doctor") => Ok(Self::Doctor),
            Some("check-connectivity") | Some("--check-connectivity") => {
                Ok(Self::CheckConnectivity)
            }
            Some("latest-incidents") | Some("--latest-incidents") => Ok(Self::LatestIncidents),
            Some("install-hint") | Some("--install-hint") | Some("install") => {
                Ok(Self::InstallHint)
            }
            Some("--help") | Some("-h") | Some("help") => Ok(Self::Help),
            Some(other) => bail!("unknown command: {other}"),
        }
    }
}

pub fn print_help() {
    println!(
        "\x1b[1mfieldmid\x1b[0m — FieldMid Edge Daemon

\x1b[1mCommands:\x1b[0m
  fieldmid                        Start the edge daemon (TUI or headless)
  fieldmid login                  Authenticate via browser or email/password
  fieldmid logout                 Clear stored session
  fieldmid whoami                 Show current auth status and org info
  fieldmid users                  List all users in your organization
  fieldmid requests               Show pending join requests for your org
    fieldmid doctor                 Diagnose env, session, DB, DNS, and connectivity
  fieldmid check-connectivity     Test PowerSync connectivity
    fieldmid latest-incidents       Show latest live incidents from local DB
    fieldmid install-hint           Print planned curl installer command
  fieldmid help                   Show this help

\x1b[1mLocal development:\x1b[0m
  cargo run --bin fieldmid
  cargo run --bin fieldmid -- login
  cargo run --bin fieldmid -- whoami
  cargo run --bin fieldmid -- users
  cargo run --bin fieldmid -- requests
    cargo run --bin fieldmid -- doctor
  cargo run --bin fieldmid -- check-connectivity
  cargo run --bin fieldmid -- latest-incidents
    cargo run --bin fieldmid -- install-hint

\x1b[1mEnvironment:\x1b[0m
  SUPABASE_URL              Supabase project URL (for auth)
  SUPABASE_ANON_KEY         Supabase anonymous key (for auth)
  POWERSYNC_URL             PowerSync instance URL
  DEVICE_ID                 Edge device identifier
  FIELDMID_DB_PATH          Local SQLite database path
  FIELDMID_DASHBOARD_URL    Core dashboard URL (for browser login)"
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
            Command::parse_arg(Some("install-hint")),
            Ok(Command::InstallHint)
        ));
        assert!(matches!(
            Command::parse_arg(Some("install")),
            Ok(Command::InstallHint)
        ));
        assert!(matches!(
            Command::parse_arg(Some("doctor")),
            Ok(Command::Doctor)
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
        assert!(matches!(
            Command::parse_arg(Some("users")),
            Ok(Command::Users)
        ));
        assert!(matches!(
            Command::parse_arg(Some("requests")),
            Ok(Command::Requests)
        ));
        assert!(matches!(
            Command::parse_arg(Some("join-requests")),
            Ok(Command::Requests)
        ));
    }
}
