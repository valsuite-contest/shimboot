// patch_rootfs binary

use clap::Parser;

#[derive(Parser)]
#[command(name = "patch_rootfs")]
#[command(about = "Patch rootfs with drivers and firmware", long_about = None)]
struct Cli {
    /// Path to shim image
    shim_path: String,
    /// Path to recovery image
    reco_path: String,
    /// Target rootfs directory
    rootfs_dir: String,
    /// Arguments in key=value format
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

fn main() {
    let cli = Cli::parse();
    let args_map = shimboot::common::parse_key_value_args(&cli.args);

    if let Err(e) = shimboot::patch_rootfs::run(&cli.shim_path, &cli.reco_path, &cli.rootfs_dir, args_map) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
