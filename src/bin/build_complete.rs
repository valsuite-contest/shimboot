// build_complete binary

use clap::Parser;

#[derive(Parser)]
#[command(name = "build_complete")]
#[command(about = "Build a complete shimboot image", long_about = None)]
struct Cli {
    /// Board name (e.g., dedede, octopus)
    board: String,
    /// Arguments in key=value format
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

fn main() {
    let cli = Cli::parse();
    let args_map = shimboot::common::parse_key_value_args(&cli.args);

    if let Err(e) = shimboot::build_complete::run(&cli.board, args_map) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
