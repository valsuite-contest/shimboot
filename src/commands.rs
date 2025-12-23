// Command handling for shimboot CLI

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
    /// Build a complete shimboot image
    BuildComplete {
        /// Board name (e.g., dedede, octopus)
        board: String,
        /// Arguments in key=value format
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Build bootloader image
    Build {
        /// Output path for the image
        output_path: String,
        /// Path to shim image
        shim_path: String,
        /// Path to rootfs directory
        rootfs_dir: String,
        /// Arguments in key=value format
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Build rootfs
    BuildRootfs {
        /// Path to rootfs directory
        rootfs_path: String,
        /// Release name (bookworm, trixie, unstable, etc.)
        release_name: String,
        /// Arguments in key=value format
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Patch rootfs with drivers and firmware
    PatchRootfs {
        /// Path to shim image
        shim_path: String,
        /// Path to recovery image
        reco_path: String,
        /// Target rootfs directory
        rootfs_dir: String,
        /// Arguments in key=value format
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Build squashfs compressed rootfs
    BuildSquashfs {
        /// Output directory
        output_dir: String,
        /// Input rootfs directory
        input_dir: String,
        /// Path to shim image
        shim_path: String,
        /// Arguments in key=value format
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

pub fn execute(command: Commands) -> Result<()> {
    match command {
        Commands::BuildComplete { board, args } => {
            let args_map = crate::common::parse_key_value_args(&args);
            crate::build_complete::run(&board, args_map)
        }
        Commands::Build {
            output_path,
            shim_path,
            rootfs_dir,
            args,
        } => {
            let args_map = crate::common::parse_key_value_args(&args);
            crate::build::run(&output_path, &shim_path, &rootfs_dir, args_map)
        }
        Commands::BuildRootfs {
            rootfs_path,
            release_name,
            args,
        } => {
            let args_map = crate::common::parse_key_value_args(&args);
            crate::build_rootfs::run(&rootfs_path, &release_name, args_map)
        }
        Commands::PatchRootfs {
            shim_path,
            reco_path,
            rootfs_dir,
            args,
        } => {
            let args_map = crate::common::parse_key_value_args(&args);
            crate::patch_rootfs::run(&shim_path, &reco_path, &rootfs_dir, args_map)
        }
        Commands::BuildSquashfs {
            output_dir,
            input_dir,
            shim_path,
            args,
        } => {
            let args_map = crate::common::parse_key_value_args(&args);
            crate::build_squashfs::run(&output_dir, &input_dir, &shim_path, args_map)
        }
    }
}

pub fn print_main_help() {
    println!("Shimboot - Boot desktop Linux from a Chrome OS RMA shim");
    println!();
    println!("Usage: shimboot <COMMAND>");
    println!();
    println!("Commands:");
    println!("  build-complete   Build a complete shimboot image");
    println!("  build            Build bootloader image");
    println!("  build-rootfs     Build rootfs");
    println!("  patch-rootfs     Patch rootfs with drivers and firmware");
    println!("  build-squashfs   Build squashfs compressed rootfs");
    println!("  help             Print this message or the help of the given subcommand(s)");
    println!();
    println!("Options:");
    println!("  -h, --help     Print help");
    println!("  -V, --version  Print version");
}
