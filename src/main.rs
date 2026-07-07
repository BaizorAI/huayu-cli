mod cli;
mod command;
mod config;
mod error;
mod services;
mod skills;
mod tool;
mod tui;

use clap::Parser;
use cli::{Cli, Commands};

fn main() {
    // Windows console defaults to a legacy code page (CP437/1252).
    // Switch both input and output to UTF-8 (CP65001) so that Chinese
    // characters, box-drawing symbols, and status icons are not garbled.
    #[cfg(windows)]
    {
        use std::os::raw::c_uint;
        extern "system" {
            fn SetConsoleOutputCP(wCodePageID: c_uint) -> i32;
            fn SetConsoleCP(wCodePageID: c_uint) -> i32;
        }
        unsafe {
            SetConsoleOutputCP(65001);
            SetConsoleCP(65001);
        }
    }

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
        Some(Commands::Update { tool }) => {
            let target = command::UpdateTarget::from_str(&tool);
            cli::commands::update::execute(target.tool_names());
            Ok(())
        }
    };

    if let Err(e) = result {
        eprintln!("错误: {}", e);
        std::process::exit(1);
    }
}
