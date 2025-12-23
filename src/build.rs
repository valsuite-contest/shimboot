// Build the bootloader image

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use crate::common::{assert_root, assert_deps, print_info, get_dir_size_mb};
use crate::{image_utils, shim_utils};

pub fn run(
    output_path: &str,
    shim_path: &str,
    rootfs_dir: &str,
    args: HashMap<String, String>,
) -> Result<()> {
    assert_root()?;
    assert_deps(&[
        "cpio", "binwalk", "pcregrep", "realpath", "cgpt", 
        "mkfs.ext4", "mkfs.ext2", "fdisk", "lz4"
    ])?;

    let output_path = Path::new(output_path);
    let shim_path = Path::new(shim_path);
    let rootfs_dir = Path::new(rootfs_dir);

    let quiet = args.get("quiet").is_some();
    let arch = args.get("arch").map(|s| s.as_str()).unwrap_or("amd64");
    let bootloader_part_name = args.get("name").map(|s| s.as_str()).unwrap_or("shimboot");
    let luks_enabled = args.get("luks").is_some();

    let crypt_password = if luks_enabled {
        print_info("downloading shimboot-binaries");
        let temp_binaries = "/tmp/shimboot-binaries.tar.gz";
        crate::common::run_command_inherit(
            "wget",
            &[
                "-q",
                "--show-progress",
                &format!("https://github.com/ading2210/shimboot-binaries/releases/latest/download/shimboot_binaries_{}.tar.gz", arch),
                "-O",
                temp_binaries,
            ],
        )?;

        crate::common::run_command_inherit(
            "tar",
            &["-xf", temp_binaries, "-C", "bootloader/bin/", "cryptsetup"],
        )?;

        std::fs::remove_file(temp_binaries)?;
        
        use std::os::unix::fs::PermissionsExt;
        let cryptsetup_path = Path::new("bootloader/bin/cryptsetup");
        let mut perms = std::fs::metadata(cryptsetup_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(cryptsetup_path, perms)?;

        // Get password from user
        use std::io::{self, Write};
        loop {
            print!("Enter the LUKS2 password for the image: ");
            io::stdout().flush()?;
            let mut password = String::new();
            io::stdin().read_line(&mut password)?;
            let password = password.trim().to_string();

            print!("Retype the password: ");
            io::stdout().flush()?;
            let mut confirm = String::new();
            io::stdin().read_line(&mut confirm)?;
            let confirm = confirm.trim().to_string();

            if password == confirm {
                break Some(password);
            } else {
                println!("Passwords do not match. Please try again.");
            }
        }
    } else {
        None
    };

    print_info("reading the shim image");
    let initramfs_dir = Path::new("/tmp/shim_initramfs");
    let kernel_img = Path::new("/tmp/kernel.img");
    
    std::fs::remove_dir_all(initramfs_dir).ok();
    std::fs::remove_file(kernel_img).ok();
    
    shim_utils::extract_initramfs_full(shim_path, initramfs_dir, Some(kernel_img), arch)?;

    print_info("patching initramfs");
    image_utils::patch_initramfs(initramfs_dir)?;

    print_info("creating disk image");
    let rootfs_size = get_dir_size_mb(rootfs_dir)?;
    let rootfs_part_size = rootfs_size * 12 / 10 + 5;
    
    image_utils::create_image(output_path, 20, rootfs_part_size, bootloader_part_name)?;

    print_info("creating loop device for the image");
    let image_loop = image_utils::create_loop(output_path)?;

    print_info("creating partitions on the disk image");
    image_utils::create_partitions(&image_loop, kernel_img, luks_enabled, crypt_password.as_deref())?;

    print_info("copying data into the image");
    image_utils::populate_partitions(&image_loop, initramfs_dir, rootfs_dir, quiet, luks_enabled)?;
    
    std::fs::remove_dir_all(initramfs_dir)?;
    std::fs::remove_file(kernel_img)?;

    print_info("cleaning up loop devices");
    crate::common::run_command_inherit("losetup", &["-d", &image_loop])?;
    
    print_info("done");

    Ok(())
}
