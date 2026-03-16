mod app;
mod banner;
mod cli;
mod config;
mod connector;
mod database;
mod schema;
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
