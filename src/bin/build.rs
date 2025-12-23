// build binary

use clap::Parser;

#[derive(Parser)]
#[command(name = "build")]
#[command(about = "Build bootloader image", long_about = None)]
struct Cli {
    /// Output path for the image
    output_path: String,
    /// Path to shim image
    shim_path: String,
    /// Path to rootfs directory
    rootfs_dir: String,
    /// Arguments in key=value format
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

fn main() {
    let cli = Cli::parse();
    let args_map = shimboot::common::parse_key_value_args(&cli.args);

    if let Err(e) = shimboot::build::run(&cli.output_path, &cli.shim_path, &cli.rootfs_dir, args_map) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
