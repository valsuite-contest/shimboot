// build_squashfs binary

use clap::Parser;

#[derive(Parser)]
#[command(name = "build_squashfs")]
#[command(about = "Build squashfs compressed rootfs", long_about = None)]
struct Cli {
    /// Output directory
    output_dir: String,
    /// Input rootfs directory
    input_dir: String,
    /// Path to shim image
    shim_path: String,
    /// Arguments in key=value format
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

fn main() {
    let cli = Cli::parse();
    let args_map = shimboot::common::parse_key_value_args(&cli.args);

    if let Err(e) = shimboot::build_squashfs::run(&cli.output_dir, &cli.input_dir, &cli.shim_path, args_map) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
