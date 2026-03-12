mod app;
mod cli;
mod config;
mod connector;
mod database;
mod demo;
mod schema;
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
        cli::Command::InsertDemoCritical => demo::insert_demo_critical().await,
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
