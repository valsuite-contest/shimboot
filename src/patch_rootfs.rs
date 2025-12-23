// Patch the target rootfs to add needed drivers

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use crate::common::{assert_root, assert_deps, run_command_inherit};
use crate::image_utils;

fn copy_modules(shim_rootfs: &Path, reco_rootfs: &Path, target_rootfs: &Path) -> Result<()> {
    let target_modules = target_rootfs.join("lib/modules");
    std::fs::remove_dir_all(&target_modules).ok();
    
    run_command_inherit(
        "cp",
        &["-r", shim_rootfs.join("lib/modules").to_str().unwrap(), target_modules.to_str().unwrap()],
    )?;

    let target_firmware = target_rootfs.join("lib/firmware");
    std::fs::create_dir_all(&target_firmware)?;
    
    run_command_inherit(
        "cp",
        &[
            "-r",
            "--remove-destination",
            &format!("{}/*", shim_rootfs.join("lib/firmware").display()),
            target_firmware.to_str().unwrap(),
        ],
    )?;

    run_command_inherit(
        "cp",
        &[
            "-r",
            "--remove-destination",
            &format!("{}/*", reco_rootfs.join("lib/firmware").display()),
            target_firmware.to_str().unwrap(),
        ],
    )?;

    // Copy modprobe configs
    let target_modprobe_lib = target_rootfs.join("lib/modprobe.d");
    let target_modprobe_etc = target_rootfs.join("etc/modprobe.d");
    std::fs::create_dir_all(&target_modprobe_lib)?;
    std::fs::create_dir_all(&target_modprobe_etc)?;

    run_command_inherit(
        "cp",
        &[
            "-r",
            &format!("{}/*", reco_rootfs.join("lib/modprobe.d").display()),
            target_modprobe_lib.to_str().unwrap(),
        ],
    ).ok();

    run_command_inherit(
        "cp",
        &[
            "-r",
            &format!("{}/*", reco_rootfs.join("etc/modprobe.d").display()),
            target_modprobe_etc.to_str().unwrap(),
        ],
    ).ok();

    // Decompress kernel modules if necessary
    let find_output = std::process::Command::new("find")
        .args([
            target_modules.to_str().unwrap(),
            "-name",
            "*.gz",
        ])
        .output()?;

    let compressed_files = String::from_utf8_lossy(&find_output.stdout);
    if !compressed_files.trim().is_empty() {
        for file in compressed_files.lines() {
            let _ = run_command_inherit("gunzip", &[file]);
        }

        // Run depmod for each kernel version
        for entry in std::fs::read_dir(&target_modules)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let version = entry.file_name().to_string_lossy().to_string();
                run_command_inherit("depmod", &["-b", target_rootfs.to_str().unwrap(), &version])?;
            }
        }
    }

    Ok(())
}

fn copy_firmware(target_rootfs: &Path) -> Result<()> {
    let firmware_path = Path::new("/tmp/chromium-firmware");

    if !firmware_path.exists() {
        download_firmware(firmware_path)?;
    }

    let target_firmware = target_rootfs.join("lib/firmware");
    run_command_inherit(
        "cp",
        &[
            "-r",
            "--remove-destination",
            &format!("{}/*", firmware_path.display()),
            target_firmware.to_str().unwrap(),
        ],
    )?;

    Ok(())
}

fn download_firmware(firmware_path: &Path) -> Result<()> {
    let firmware_url = "https://chromium.googlesource.com/chromiumos/third_party/linux-firmware";
    
    run_command_inherit(
        "git",
        &[
            "clone",
            "--branch",
            "master",
            "--depth=1",
            firmware_url,
            firmware_path.to_str().unwrap(),
        ],
    )?;

    Ok(())
}

pub fn run(
    shim_path: &str,
    reco_path: &str,
    target_rootfs: &str,
    _args: HashMap<String, String>,
) -> Result<()> {
    assert_root()?;
    assert_deps(&["git", "gunzip", "depmod"])?;

    let shim_path = Path::new(shim_path);
    let reco_path = Path::new(reco_path);
    let target_rootfs = Path::new(target_rootfs);
    let shim_rootfs = Path::new("/tmp/shim_rootfs");
    let reco_rootfs = Path::new("/tmp/reco_rootfs");

    println!("mounting shim");
    let shim_loop = image_utils::create_loop(shim_path)?;
    let shim_p3 = format!("{}p3", shim_loop);
    image_utils::safe_mount(&shim_p3, shim_rootfs, Some("ro"))?;

    println!("mounting recovery image");
    let reco_loop = image_utils::create_loop(reco_path)?;
    let reco_p3 = format!("{}p3", reco_loop);
    image_utils::safe_mount(&reco_p3, reco_rootfs, Some("ro"))?;

    println!("copying modules to rootfs");
    copy_modules(shim_rootfs, reco_rootfs, target_rootfs)?;

    println!("downloading misc firmware");
    copy_firmware(target_rootfs)?;

    println!("unmounting and cleaning up");
    run_command_inherit("umount", &[shim_rootfs.to_str().unwrap()])?;
    run_command_inherit("umount", &[reco_rootfs.to_str().unwrap()])?;
    run_command_inherit("losetup", &["-d", &shim_loop])?;
    run_command_inherit("losetup", &["-d", &reco_loop])?;

    println!("done");

    Ok(())
}
