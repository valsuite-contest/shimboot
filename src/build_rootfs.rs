// Build the debian/ubuntu/alpine rootfs

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use crate::common::{assert_root, assert_deps, run_command_inherit, print_info};

pub fn run(
    rootfs_path: &str,
    release_name: &str,
    args: HashMap<String, String>,
) -> Result<()> {
    assert_root()?;
    assert_deps(&["realpath", "debootstrap", "findmnt", "wget", "pcregrep", "tar"])?;

    let rootfs_dir = Path::new(rootfs_path);
    let packages = args.get("custom_packages")
        .map(|s| s.as_str())
        .unwrap_or("task-xfce-desktop");
    let arch = args.get("arch").map(|s| s.as_str()).unwrap_or("amd64");
    let distro = args.get("distro").map(|s| s.as_str()).unwrap_or("debian");

    std::fs::create_dir_all(rootfs_dir)?;

    // Check if we need to remount
    check_and_remount(rootfs_dir)?;

    let chroot_script = if distro == "alpine" {
        bootstrap_alpine(rootfs_dir, release_name, arch)?;
        "/opt/setup_rootfs_alpine.sh"
    } else {
        bootstrap_debian_ubuntu(rootfs_dir, release_name, arch, distro)?;
        "/opt/setup_rootfs.sh"
    };

    print_info("copying rootfs setup scripts");
    run_command_inherit("cp", &["-arv", "rootfs/*", rootfs_dir.to_str().unwrap()])?;
    run_command_inherit(
        "cp",
        &["/etc/resolv.conf", &format!("{}/etc/resolv.conf", rootfs_dir.display())],
    )?;

    print_info("creating bind mounts for chroot");
    let chroot_mounts = ["proc", "sys", "dev", "run"];
    for mountpoint in &chroot_mounts {
        let target = rootfs_dir.join(mountpoint);
        run_command_inherit(
            "mount",
            &["--make-rslave", "--rbind", &format!("/{}", mountpoint), target.to_str().unwrap()],
        )?;
    }

    // Build chroot command
    let hostname = args.get("hostname").map(|s| s.as_str()).unwrap_or("");
    let root_passwd = args.get("root_passwd").map(|s| s.as_str()).unwrap_or("");
    let enable_root = args.get("enable_root").map(|s| s.as_str()).unwrap_or("");
    let username = args.get("username").map(|s| s.as_str()).unwrap_or("");
    let user_passwd = args.get("user_passwd").map(|s| s.as_str()).unwrap_or("");
    let disable_base = args.get("disable_base").map(|s| s.as_str()).unwrap_or("");

    let chroot_command = format!(
        "{} '{}' '{}' '{}' '{}' '{}' '{}' '{}' '{}' '{}' '{}'",
        chroot_script,
        std::env::var("DEBUG").unwrap_or_default(),
        release_name,
        packages,
        hostname,
        root_passwd,
        username,
        user_passwd,
        enable_root,
        disable_base,
        arch
    );

    // Run chroot
    let status = std::process::Command::new("chroot")
        .arg(rootfs_dir)
        .arg("/bin/sh")
        .arg("-c")
        .arg(&chroot_command)
        .env("LC_ALL", "C")
        .status()?;

    if !status.success() {
        anyhow::bail!("Chroot command failed");
    }

    // Unmount
    for mountpoint in &chroot_mounts {
        let target = rootfs_dir.join(mountpoint);
        let _ = run_command_inherit("umount", &["-l", target.to_str().unwrap()]);
    }

    print_info("rootfs has been created");

    Ok(())
}

fn check_and_remount(rootfs_dir: &Path) -> Result<()> {
    let findmnt_output = std::process::Command::new("findmnt")
        .args(["-T", rootfs_dir.to_str().unwrap()])
        .output()?;

    let output = String::from_utf8_lossy(&findmnt_output.stdout);
    let last_line = output.lines().last().unwrap_or("");
    let mnt_options = last_line.split_whitespace().last().unwrap_or("");

    if mnt_options.contains("noexec") || mnt_options.contains("nodev") {
        let mountpoint = last_line.split_whitespace().next().unwrap_or("");
        run_command_inherit("mount", &["-o", "remount,dev,exec", mountpoint])?;
    }

    Ok(())
}

fn bootstrap_debian_ubuntu(rootfs_dir: &Path, release: &str, arch: &str, distro: &str) -> Result<()> {
    print_info(&format!("bootstraping {} chroot", distro));

    let repo_url = if distro == "ubuntu" {
        if arch == "amd64" {
            "http://archive.ubuntu.com/ubuntu"
        } else {
            "http://ports.ubuntu.com"
        }
    } else {
        "http://deb.debian.org/debian/"
    };

    let mut args = vec![
        "--arch", arch,
        release,
        rootfs_dir.to_str().unwrap(),
        repo_url,
    ];

    if distro == "debian" {
        args.splice(1..1, vec!["--components=main,contrib,non-free,non-free-firmware"]);
    }

    run_command_inherit("debootstrap", &args)?;

    Ok(())
}

fn bootstrap_alpine(rootfs_dir: &Path, release: &str, arch: &str) -> Result<()> {
    print_info("downloading alpine package list");
    let pkg_list_url = "https://dl-cdn.alpinelinux.org/alpine/latest-stable/main/x86_64/";
    let pkg_data = reqwest::blocking::get(pkg_list_url)
        .context("Failed to download Alpine package list")?
        .text()?;

    let re = regex::Regex::new(r#""(.+?apk-tools-static.+?\.apk)""#)?;
    let pkg_name = re.captures(&pkg_data)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
        .context("Could not find apk-tools-static package")?;

    let pkg_url = format!("{}{}", pkg_list_url, pkg_name);

    print_info("downloading and extracting apk-tools-static");
    let pkg_extract_dir = Path::new("/tmp/apk-tools-static");
    std::fs::create_dir_all(pkg_extract_dir)?;
    
    let pkg_dl_path = pkg_extract_dir.join("pkg.apk");
    let mut file = std::fs::File::create(&pkg_dl_path)?;
    let mut response = reqwest::blocking::get(&pkg_url)?;
    std::io::copy(&mut response, &mut file)?;

    run_command_inherit(
        "tar",
        &["--warning=no-unknown-keyword", "-xzf", pkg_dl_path.to_str().unwrap(), "-C", pkg_extract_dir.to_str().unwrap()],
    )?;

    let apk_static = pkg_extract_dir.join("sbin/apk.static");

    print_info("bootstraping alpine chroot");
    let real_arch = if arch == "arm64" { "aarch64" } else { "x86_64" };

    run_command_inherit(
        apk_static.to_str().unwrap(),
        &[
            &format!("--arch={}", real_arch),
            &format!("-X=http://dl-cdn.alpinelinux.org/alpine/{}/main/", release),
            "-U",
            "--allow-untrusted",
            &format!("--root={}", rootfs_dir.display()),
            "--initdb",
            "add",
            "alpine-base",
        ],
    )?;

    Ok(())
}
