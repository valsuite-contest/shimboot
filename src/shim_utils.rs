// Utilities for reading and extracting shim disk images

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use crate::common::run_command_inherit;
use crate::image_utils;

/// Run binwalk with appropriate flags
fn run_binwalk(args: &[&str]) -> Result<String> {
    let help_output = Command::new("binwalk")
        .arg("-h")
        .output()?;

    let help = String::from_utf8_lossy(&help_output.stdout);
    let mut cmd = Command::new("binwalk");
    
    for arg in args {
        cmd.arg(arg);
    }

    if help.contains("--run-as") {
        cmd.arg("--run-as=root");
    }

    let output = cmd.output()?;
    
    if !output.status.success() {
        anyhow::bail!("binwalk failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Extract initramfs from a kernel image (x86_64)
pub fn extract_initramfs(
    kernel_bin: &Path,
    working_dir: &Path,
    output_dir: &Path,
) -> Result<()> {
    std::fs::create_dir_all(working_dir)?;
    
    let kernel_file = kernel_bin.file_name().unwrap().to_str().unwrap();
    
    // Extract the compressed kernel image
    let binwalk_out = run_binwalk(&[
        "--extract",
        kernel_bin.to_str().unwrap(),
        &format!("--directory={}", working_dir.display()),
    ])?;

    // Parse output to find gzip offset
    let stage1_file = binwalk_out
        .lines()
        .find(|l| l.contains("gzip compressed data"))
        .and_then(|l| {
            let parts: Vec<&str> = l.split_whitespace().collect();
            if parts.len() >= 2 {
                parts[0].parse::<u32>().ok().map(|n| format!("{:X}", n))
            } else {
                None
            }
        })
        .context("Could not find gzip compressed data in kernel")?;

    let stage1_dir = working_dir.join(format!("_{}.extracted", kernel_file));
    let stage1_path = stage1_dir.join(&stage1_file);

    // Extract the initramfs cpio archive from the kernel image
    run_binwalk(&[
        "--extract",
        stage1_path.to_str().unwrap(),
        &format!("--directory={}", stage1_dir.display()),
    ])?;

    let stage2_dir = stage1_dir.join(format!("_{}.extracted", stage1_file));
    
    // Find the cpio file
    let mut cpio_file = None;
    for entry in std::fs::read_dir(&stage2_dir)? {
        let entry = entry?;
        let output = Command::new("file")
            .arg(entry.path())
            .output()?;
        let file_output = String::from_utf8_lossy(&output.stdout);
        if file_output.contains("ASCII cpio archive") {
            cpio_file = Some(entry.file_name().to_string_lossy().to_string());
            break;
        }
    }

    let cpio_path = stage2_dir.join(cpio_file.context("Could not find cpio archive")?);

    // Extract cpio archive
    std::fs::remove_dir_all(output_dir).ok();
    std::fs::create_dir_all(output_dir)?;

    let cpio_data = std::fs::read(&cpio_path)?;
    let mut child = Command::new("cpio")
        .args(["-D", output_dir.to_str().unwrap(), "-imd", "--quiet"])
        .stdin(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(&cpio_data)?;
    }

    child.wait()?;

    Ok(())
}

/// Extract initramfs from a kernel image (ARM)
pub fn extract_initramfs_arm(
    kernel_bin: &Path,
    working_dir: &Path,
    output_dir: &Path,
) -> Result<()> {
    std::fs::create_dir_all(working_dir)?;

    // Extract the kernel lz4 archive from the partition
    let binwalk_out = run_binwalk(&[kernel_bin.to_str().unwrap()])?;
    
    let lz4_offset = binwalk_out
        .lines()
        .find(|l| l.contains("LZ4 compressed data"))
        .and_then(|l| {
            l.split_whitespace()
                .next()
                .and_then(|s| s.parse::<u64>().ok())
        })
        .context("Could not find LZ4 compressed data")?;

    let lz4_file = working_dir.join("kernel.lz4");
    let kernel_img = working_dir.join("kernel_decompressed.bin");

    run_command_inherit(
        "dd",
        &[
            &format!("if={}", kernel_bin.display()),
            &format!("of={}", lz4_file.display()),
            "iflag=skip_bytes,count_bytes",
            &format!("skip={}", lz4_offset),
        ],
    )?;

    let _ = run_command_inherit(
        "lz4",
        &[
            "-d",
            lz4_file.to_str().unwrap(),
            kernel_img.to_str().unwrap(),
            "-q",
        ],
    );

    // Extract the initramfs cpio archive
    let extracted_dir = working_dir.join("_kernel_decompressed.bin.extracted");
    run_binwalk(&[
        "--extract",
        kernel_img.to_str().unwrap(),
        &format!("--directory={}", working_dir.display()),
    ])?;

    // Find the cpio file
    let mut cpio_file = None;
    for entry in std::fs::read_dir(&extracted_dir)? {
        let entry = entry?;
        let output = Command::new("file")
            .arg(entry.path())
            .output()?;
        let file_output = String::from_utf8_lossy(&output.stdout);
        if file_output.contains("ASCII cpio archive") {
            cpio_file = Some(entry.file_name().to_string_lossy().to_string());
            break;
        }
    }

    let cpio_path = extracted_dir.join(cpio_file.context("Could not find cpio archive")?);

    // Extract cpio archive
    std::fs::remove_dir_all(output_dir).ok();
    std::fs::create_dir_all(output_dir)?;

    let cpio_data = std::fs::read(&cpio_path)?;
    let mut child = Command::new("cpio")
        .args(["-D", output_dir.to_str().unwrap(), "-imd", "--quiet"])
        .stdin(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(&cpio_data)?;
    }

    child.wait()?;

    Ok(())
}

/// Copy the kernel from shim
pub fn copy_kernel(shim_path: &Path, kernel_dir: &Path) -> Result<()> {
    let shim_loop = image_utils::create_loop(shim_path)?;
    let kernel_loop = format!("{}p2", shim_loop); // KERN-A should always be p2

    run_command_inherit(
        "dd",
        &[
            &format!("if={}", kernel_loop),
            &format!("of={}", kernel_dir.join("kernel.bin").display()),
            "bs=1M",
            "status=progress",
        ],
    )?;

    run_command_inherit("losetup", &["-d", &shim_loop])?;

    Ok(())
}

/// Extract initramfs from shim (full process)
pub fn extract_initramfs_full(
    shim_path: &Path,
    rootfs_dir: &Path,
    kernel_bin: Option<&Path>,
    arch: &str,
) -> Result<()> {
    let kernel_dir = PathBuf::from("/tmp/shim_kernel");

    println!("copying the shim kernel");
    std::fs::remove_dir_all(&kernel_dir).ok();
    std::fs::create_dir_all(&kernel_dir)?;
    copy_kernel(shim_path, &kernel_dir)?;

    println!("extracting initramfs from kernel (this may take a while)");
    let kernel_path = kernel_dir.join("kernel.bin");
    
    if arch == "arm64" {
        extract_initramfs_arm(&kernel_path, &kernel_dir, rootfs_dir)?;
    } else {
        extract_initramfs(&kernel_path, &kernel_dir, rootfs_dir)?;
    }

    if let Some(kernel_out) = kernel_bin {
        std::fs::copy(&kernel_path, kernel_out)?;
    }

    std::fs::remove_dir_all(&kernel_dir)?;

    Ok(())
}
