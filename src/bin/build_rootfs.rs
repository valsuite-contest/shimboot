// build_rootfs binary

use clap::Parser;

#[derive(Parser)]
#[command(name = "build_rootfs")]
#[command(about = "Build rootfs", long_about = None)]
struct Cli {
    /// Path to rootfs directory
    rootfs_path: String,
    /// Release name (bookworm, trixie, unstable, etc.)
    release_name: String,
    /// Arguments in key=value format
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

fn main() {
    let cli = Cli::parse();
    let args_map = shimboot::common::parse_key_value_args(&cli.args);

    if let Err(e) = shimboot::build_rootfs::run(&cli.rootfs_path, &cli.release_name, args_map) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
