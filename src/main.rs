mod cli;
mod command;
mod config;
mod error;
mod services;
mod tool;
mod tui;

use clap::Parser;
use cli::{Cli, Commands};

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        None => {
            let config = config::load();
            tui::run(config).map_err(|e| error::AppError::Message(e.to_string()))
        }
        Some(Commands::Login(args)) => cli::commands::login::execute(args),
        Some(Commands::Status) => {
            cli::commands::status::execute();
            Ok(())
        }
    };

    if let Err(e) = result {
        eprintln!("错误: {}", e);
        std::process::exit(1);
    }
}
