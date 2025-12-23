// Common utilities for shimboot

use anyhow::{Context, Result};
use colored::Colorize;
use std::collections::HashMap;
use std::process::{Command, Stdio};

/// Parse key=value arguments into a HashMap
pub fn parse_key_value_args(args: &[String]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for arg in args {
        if let Some((key, value)) = arg.split_once('=') {
            map.insert(key.to_string(), value.to_string());
        }
    }
    map
}

/// Check if running as root
pub fn assert_root() -> Result<()> {
    if !nix::unistd::Uid::effective().is_root() {
        anyhow::bail!("This script needs to be run as root.");
    }
    Ok(())
}

/// Check if a command exists in PATH
pub fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Check if required dependencies are available
pub fn check_deps(commands: &[&str]) -> Vec<String> {
    commands
        .iter()
        .filter(|cmd| !command_exists(cmd))
        .map(|s| s.to_string())
        .collect()
}

/// Assert that all dependencies are available
pub fn assert_deps(commands: &[&str]) -> Result<()> {
    let missing = check_deps(commands);
    if !missing.is_empty() {
        eprintln!("{}", "You are missing dependencies needed for this script.".red());
        eprintln!("{}", "Commands needed:".red());
        for cmd in missing {
            eprintln!("  - {}", cmd);
        }
        anyhow::bail!("Missing dependencies");
    }
    Ok(())
}

/// Print a title message (green)
pub fn print_title(msg: &str) {
    println!("{}", format!(">> {}", msg).green());
}

/// Print an info message (bold)
pub fn print_info(msg: &str) {
    println!("{}", msg.bold());
}

/// Print an error message (red)
pub fn print_error(msg: &str) {
    eprintln!("{}", msg.red());
}

/// Get the git tag/hash for version information
pub fn get_git_version() -> (Option<String>, Option<String>) {
    let tag = Command::new("git")
        .args(["tag", "-l", "--contains", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        });

    let hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    (tag, hash)
}

/// Run a command and return its output
pub fn run_command(cmd: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .with_context(|| format!("Failed to execute {}", cmd))?;

    if !output.status.success() {
        anyhow::bail!(
            "Command {} failed: {}",
            cmd,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Run a command without capturing output
pub fn run_command_inherit(cmd: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(cmd)
        .args(args)
        .status()
        .with_context(|| format!("Failed to execute {}", cmd))?;

    if !status.success() {
        anyhow::bail!("Command {} failed with exit code {:?}", cmd, status.code());
    }

    Ok(())
}

/// Get the directory size in MB
pub fn get_dir_size_mb(path: &std::path::Path) -> Result<u64> {
    let output = Command::new("du")
        .args(["-sm", path.to_str().unwrap()])
        .output()
        .context("Failed to get directory size")?;

    let size_str = String::from_utf8_lossy(&output.stdout);
    let size = size_str
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<u64>().ok())
        .context("Failed to parse directory size")?;

    Ok(size)
}

/// Copy directory with progress using tar and pv
pub fn copy_with_progress(source: &std::path::Path, dest: &std::path::Path, quiet: bool) -> Result<()> {
    std::fs::create_dir_all(dest)?;

    if quiet {
        run_command_inherit("cp", &["-ar", source.to_str().unwrap(), dest.to_str().unwrap()])?;
    } else {
        // Use tar + pv for progress
        let total_bytes = Command::new("du")
            .args(["-sb", source.to_str().unwrap()])
            .output()?;
        let size = String::from_utf8_lossy(&total_bytes.stdout)
            .split_whitespace()
            .next()
            .unwrap_or("0")
            .to_string();

        let tar_src = Command::new("tar")
            .args(["-cf", "-", "-C", source.to_str().unwrap(), "."])
            .stdout(Stdio::piped())
            .spawn()?;

        let pv = Command::new("pv")
            .args(["-f", "-s", &size])
            .stdin(tar_src.stdout.unwrap())
            .stdout(Stdio::piped())
            .spawn()?;

        let status = Command::new("tar")
            .args(["-xf", "-", "-C", dest.to_str().unwrap()])
            .stdin(pv.stdout.unwrap())
            .status()?;

        if !status.success() {
            anyhow::bail!("Failed to copy with progress");
        }
    }

    Ok(())
}
