mod app;
mod auth;
mod banner;
mod cli;
mod config;
mod connector;
mod database;
mod network;
mod schema;
mod session;
mod tui;
mod watcher;

#[tokio::main]
async fn main() {
    let command = match cli::Command::parse() {
        Ok(command) => command,
        Err(error) => {
            eprintln!("\n  \x1b[31m✗ {error:#}\x1b[0m\n");
            cli::print_help();
            std::process::exit(1);
        }
    };

    let result = match command {
        cli::Command::Run => app::run().await,
        cli::Command::Login => app::login().await,
        cli::Command::Logout => app::logout(),
        cli::Command::WhoAmI => app::whoami().await,
        cli::Command::Doctor => app::doctor().await,
        cli::Command::CheckConnectivity => app::check_connectivity().await,
        cli::Command::LatestIncidents => app::show_latest_incidents().await,
        cli::Command::InstallHint => app::print_install_hint(),
        cli::Command::Help => {
            cli::print_help();
            Ok(())
        }
    };

    if let Err(error) = result {
        // Error messages from auth.rs are already formatted with ANSI colors
        let msg = format!("{error:#}");
        if msg.contains('\x1b') {
            // Already formatted
            eprintln!("\n{msg}\n");
        } else {
            eprintln!("\n  \x1b[31m✗ {msg}\x1b[0m\n");
        }
        std::process::exit(1);
    }
}
