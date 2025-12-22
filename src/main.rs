// Shimboot - Boot desktop Linux from a Chrome OS RMA shim
// Copyright (C) 2025 ading2210
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use clap::Parser;

#[derive(Parser)]
#[command(name = "shimboot")]
#[command(about = "Boot desktop Linux from a Chrome OS RMA shim", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<shimboot::commands::Commands>,
}

fn main() {
    let cli = Cli::parse();

    let result = if let Some(command) = cli.command {
        shimboot::commands::execute(command)
    } else {
        shimboot::commands::print_main_help();
        Ok(())
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
