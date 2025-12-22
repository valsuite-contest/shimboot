// Image utilities for disk operations

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use crate::common::{run_command, run_command_inherit};

/// Create a loop device for an image file
pub fn create_loop(image_path: &Path) -> Result<String> {
    // Find available loop device
    let loop_device = run_command("losetup", &["-f"])?
        .trim()
        .to_string();

    // Check if device exists, create if necessary
    if !Path::new(&loop_device).exists() {
        let proc_devices = std::fs::read_to_string("/proc/devices")?;
        let major = proc_devices
            .lines()
            .find(|l| l.contains("loop"))
            .and_then(|l| l.split_whitespace().next())
            .context("Could not find loop device major number")?;

        let number = loop_device
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect::<String>();

        run_command_inherit("mknod", &[&loop_device, "b", major, &number])?;
    }

    run_command_inherit("losetup", &["-P", &loop_device, image_path.to_str().unwrap()])?;

    Ok(loop_device)
}

/// Set required flags on the kernel partition
pub fn make_bootable(image_loop: &str) -> Result<()> {
    run_command_inherit(
        "cgpt",
        &["add", "-i", "2", "-S", "1", "-T", "5", "-P", "10", "-l", "kernel", image_loop],
    )
}

/// Partition the disk image using fdisk
pub fn partition_disk(image_path: &Path, bootloader_size: u64, rootfs_name: &str) -> Result<()> {
    let fdisk_commands = format!(
        "g\n\
         n\n\n\n+1M\n\
         n\n\n\n+32M\n\
         t\n\nFE3A2A5D-4F32-41A7-B725-ACCC3285A309\n\
         n\n\n\n+{}M\n\
         t\n\n3CB8E202-3B7E-47DD-8A3C-7FF2A13CFCEC\n\
         n\n\n\n\n\
         x\nn\n\nshimboot_rootfs:{}\n\
         r\nw\n",
        bootloader_size, rootfs_name
    );

    let mut child = Command::new("fdisk")
        .arg(image_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(fdisk_commands.as_bytes())?;
    }

    let status = child.wait()?;
    if !status.success() {
        anyhow::bail!("fdisk failed");
    }

    Ok(())
}

/// Safe mount - unmount first if needed, create directory, then mount
pub fn safe_mount(source: &str, dest: &Path, opts: Option<&str>) -> Result<()> {
    // Try to unmount first
    let _ = Command::new("umount").arg(dest).status();

    std::fs::remove_dir_all(dest).ok();
    std::fs::create_dir_all(dest)?;

    if let Some(options) = opts {
        run_command_inherit("mount", &[source, dest.to_str().unwrap(), "-o", options])?;
    } else {
        run_command_inherit("mount", &[source, dest.to_str().unwrap()])?;
    }

    Ok(())
}

/// Create partitions on the disk image
pub fn create_partitions(
    image_loop: &str,
    kernel_path: &Path,
    is_luks: bool,
    crypt_password: Option<&str>,
) -> Result<()> {
    let p1 = format!("{}p1", image_loop);
    let p2 = format!("{}p2", image_loop);
    let p3 = format!("{}p3", image_loop);
    let p4 = format!("{}p4", image_loop);

    // Create stateful partition
    run_command_inherit("mkfs.ext4", &[&p1])?;

    // Copy kernel
    run_command_inherit(
        "dd",
        &[
            &format!("if={}", kernel_path.display()),
            &format!("of={}", p2),
            "bs=1M",
            "oflag=sync",
        ],
    )?;

    make_bootable(image_loop)?;

    // Create bootloader partition
    run_command_inherit("mkfs.ext2", &[&p3])?;

    // Create rootfs partition
    if is_luks {
        let password = crypt_password.context("LUKS password required")?;
        
        let mut child = Command::new("cryptsetup")
            .args(["luksFormat", &p4])
            .stdin(std::process::Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            writeln!(stdin, "{}", password)?;
        }
        child.wait()?;

        let mut child = Command::new("cryptsetup")
            .args(["luksOpen", &p4, "rootfs"])
            .stdin(std::process::Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            writeln!(stdin, "{}", password)?;
        }
        child.wait()?;

        run_command_inherit("mkfs.ext4", &["/dev/mapper/rootfs"])?;
    } else {
        run_command_inherit("mkfs.ext4", &[&p4])?;
    }

    Ok(())
}

/// Populate partitions with data
pub fn populate_partitions(
    image_loop: &str,
    bootloader_dir: &Path,
    rootfs_dir: &Path,
    quiet: bool,
    luks_enabled: bool,
) -> Result<()> {
    let (git_tag, git_hash) = crate::common::get_git_version();

    let p1 = format!("{}p1", image_loop);
    let p3 = format!("{}p3", image_loop);
    let p4 = format!("{}p4", image_loop);

    // Mount and write to stateful
    let stateful_mount = PathBuf::from("/tmp/shim_stateful");
    safe_mount(&p1, &stateful_mount, None)?;
    std::fs::create_dir_all(stateful_mount.join("dev_image/etc"))?;
    std::fs::create_dir_all(stateful_mount.join("dev_image/factory/sh"))?;
    std::fs::File::create(stateful_mount.join("dev_image/etc/lsb-factory"))?;
    run_command_inherit("umount", &[stateful_mount.to_str().unwrap()])?;

    // Mount and write to bootloader
    let bootloader_mount = PathBuf::from("/tmp/shim_bootloader");
    safe_mount(&p3, &bootloader_mount, None)?;
    
    run_command_inherit(
        "cp",
        &["-arv", &format!("{}/*", bootloader_dir.display()), bootloader_mount.to_str().unwrap()],
    )?;

    if git_tag.is_none() {
        if let Some(hash) = git_hash {
            std::fs::write(
                bootloader_mount.join("opt/.shimboot_version_dev"),
                hash,
            )?;
        }
    }

    run_command_inherit("umount", &[bootloader_mount.to_str().unwrap()])?;

    // Write rootfs to image
    let rootfs_mount = PathBuf::from("/tmp/new_rootfs");
    if luks_enabled {
        safe_mount("/dev/mapper/rootfs", &rootfs_mount, None)?;
    } else {
        safe_mount(&p4, &rootfs_mount, None)?;
    }

    if quiet {
        run_command_inherit(
            "cp",
            &["-ar", &format!("{}/*", rootfs_dir.display()), rootfs_mount.to_str().unwrap()],
        )?;
    } else {
        crate::common::copy_with_progress(rootfs_dir, &rootfs_mount, quiet)?;
    }

    run_command_inherit("umount", &[rootfs_mount.to_str().unwrap()])?;

    if luks_enabled {
        run_command_inherit("cryptsetup", &["close", "rootfs"])?;
    }

    Ok(())
}

/// Create a disk image
pub fn create_image(
    image_path: &Path,
    bootloader_size: u64,
    rootfs_size: u64,
    rootfs_name: &str,
) -> Result<()> {
    let total_size = 1 + 32 + bootloader_size + rootfs_size;

    if image_path.exists() {
        std::fs::remove_file(image_path)?;
    }

    run_command_inherit(
        "fallocate",
        &["-l", &format!("{}M", total_size), image_path.to_str().unwrap()],
    )?;

    partition_disk(image_path, bootloader_size, rootfs_name)?;

    Ok(())
}

/// Patch the initramfs
pub fn patch_initramfs(initramfs_path: &Path) -> Result<()> {
    let init_path = initramfs_path.join("init");
    if init_path.exists() {
        std::fs::remove_file(&init_path)?;
    }

    run_command_inherit(
        "cp",
        &["-r", "bootloader/*", initramfs_path.to_str().unwrap()],
    )?;

    // Make all binaries executable
    let bin_path = initramfs_path.join("bin");
    if bin_path.exists() {
        for entry in walkdir::WalkDir::new(&bin_path) {
            let entry = entry?;
            if entry.file_type().is_file() {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = entry.metadata()?.permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(entry.path(), perms)?;
            }
        }
    }

    Ok(())
}

/// Clean up unused loop devices
pub fn clean_loops() -> Result<()> {
    let losetup_output = Command::new("losetup")
        .arg("-a")
        .output()?;

    let output = String::from_utf8_lossy(&losetup_output.stdout);
    for line in output.lines() {
        if let Some(device) = line.split(':').next() {
            // Check if device is mounted
            let mounts = std::fs::read_to_string("/proc/mounts")?;
            if !mounts.contains(device) {
                let _ = run_command_inherit("losetup", &["-d", device]);
            }
        }
    }

    Ok(())
}
