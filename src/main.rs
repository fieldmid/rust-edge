mod app;
mod auth;
mod banner;
mod cli;
mod config;
mod connector;
mod database;
mod schema;
mod session;
mod tui;
mod watcher;

#[tokio::main]
async fn main() {
    let command = match cli::Command::parse() {
        Ok(command) => command,
        Err(error) => {
            eprintln!("{error:#}");
            cli::print_help();
            std::process::exit(1);
        }
    };

    let result = match command {
        cli::Command::Run => app::run().await,
        cli::Command::Login => app::login().await,
        cli::Command::Logout => app::logout(),
        cli::Command::WhoAmI => app::whoami().await,
        cli::Command::CheckConnectivity => app::check_connectivity().await,
        cli::Command::LatestIncidents => app::show_latest_incidents().await,
        cli::Command::Help => {
            cli::print_help();
            Ok(())
        }
    };

    if let Err(error) = result {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
