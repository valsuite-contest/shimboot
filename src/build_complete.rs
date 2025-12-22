// Complete build script - downloads images and builds shimboot

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::io::Write;
use crate::common::{assert_root, assert_deps, check_deps, print_title, print_info, print_error, run_command_inherit};

const ARM_BOARDS: &[&str] = &[
    "corsola", "hana", "jacuzzi", "kukui", "strongbad", "nyan-big", "kevin", "bob",
    "veyron-speedy", "veyron-jerry", "veyron-minnie", "scarlet", "elm",
    "kukui", "peach-pi", "peach-pit", "stumpy", "daisy-spring", "trogdor",
];

const BAD_BOARDS: &[&str] = &["reef", "sand", "pyro"];

pub fn run(board: &str, args: HashMap<String, String>) -> Result<()> {
    assert_root()?;

    let compress_img = args.get("compress_img").is_some();
    let rootfs_dir = args.get("rootfs_dir").map(|s| PathBuf::from(s));
    let quiet = args.get("quiet").is_some();
    let desktop = args.get("desktop").unwrap_or(&"xfce".to_string()).clone();
    let data_dir = args.get("data_dir")
        .map(|s| PathBuf::from(s))
        .unwrap_or_else(|| PathBuf::from("./data"));
    let mut arch = args.get("arch").unwrap_or(&"amd64".to_string()).clone();
    let release = args.get("release").cloned();
    let distro = args.get("distro").unwrap_or(&"debian".to_string()).clone();
    let luks = args.get("luks").is_some();

    // Auto-detect ARM boards
    if ARM_BOARDS.contains(&board) {
        print_info("automatically detected arm64 device name");
        arch = "arm64".to_string();
    }

    // Warn about bad boards
    if BAD_BOARDS.contains(&board) {
        print_error("Warning: you are attempting to build Shimboot for a board which has a shim that includes a fix for the sh1mmer vulnerability. The resulting image will not boot if you are enrolled.");
        print!("Press [enter] to continue ");
        std::io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
    }

    // Check LUKS compatibility
    if luks && arch == "arm64" {
        print_error("Uh-oh, you are trying to use luks2 encryption on an arm64 board. Unfortunately, rootfs encryption is not available on arm64-based boards at this time. :(");
        anyhow::bail!("LUKS not supported on ARM64");
    }

    // Check dependencies
    let needed_deps = [
        "wget", "python3", "unzip", "zip", "git", "debootstrap", "cpio",
        "binwalk", "pcregrep", "cgpt", "mkfs.ext4", "mkfs.ext2", "fdisk",
        "depmod", "findmnt", "lz4", "pv", "cryptsetup",
    ];

    if !check_deps(&needed_deps).is_empty() {
        if Path::new("/etc/debian_version").exists() {
            print_title("attempting to install build deps");
            run_command_inherit(
                "apt-get",
                &[
                    "install", "-y", "wget", "python3", "unzip", "zip",
                    "debootstrap", "cpio", "binwalk", "pcregrep", "cgpt",
                    "kmod", "pv", "lz4", "cryptsetup",
                ],
            )?;
        }
        assert_deps(&needed_deps)?;
    }

    // Install qemu-user-static if needed for cross-architecture builds
    let kernel_arch = std::env::consts::ARCH;
    let host_arch = match kernel_arch {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        _ => "unknown",
    };

    if arch != host_arch && Path::new("/etc/debian_version").exists() {
        let dpkg_output = std::process::Command::new("dpkg")
            .args(["--get-selections"])
            .output()?;
        let selections = String::from_utf8_lossy(&dpkg_output.stdout);
        
        if !selections.contains("qemu-user-static") 
            && !selections.contains("box64") 
            && !selections.contains("fex-emu") {
            print_info("automatically installing qemu-user-static because we are building for a different architecture");
            run_command_inherit("apt-get", &["install", "-y", "qemu-user-static", "binfmt-support"])?;
        }
    } else if arch != host_arch {
        print_error("Warning: You are building an image for a different CPU architecture. It may fail if you do not have qemu-user-static installed.");
        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    std::fs::create_dir_all(&data_dir)?;

    // Download recovery image
    print_title("downloading list of recovery images");
    let boards_url = "https://chromiumdash.appspot.com/cros/fetch_serving_builds?deviceCategory=ChromeOS";
    
    let boards_json = reqwest::blocking::get(boards_url)
        .context("Failed to download boards list")?
        .text()?;

    let boards: serde_json::Value = serde_json::from_str(&boards_json)?;
    let reco_url = find_recovery_url(&boards, board)?;
    print_info(&format!("found url: {}", reco_url));

    let shim_bin = data_dir.join(format!("shim_{}.bin", board));
    let reco_bin = data_dir.join(format!("reco_{}.bin", board));
    let reco_zip = data_dir.join(format!("reco_{}.zip", board));

    print_title("downloading recovery image");
    download_and_unzip(&reco_url, &reco_zip, &reco_bin, quiet)?;

    print_title("downloading shim image");
    if !shim_bin.exists() {
        download_shim(board, &data_dir, &shim_bin, quiet)?;
    }

    // Build rootfs
    print_title(&format!("building {} rootfs", distro));
    let rootfs_dir = if let Some(dir) = rootfs_dir {
        dir
    } else {
        let desktop_package = format!("task-{}-desktop", desktop);
        let rootfs_dir = data_dir.join(format!("rootfs_{}", board));

        // Unmount if necessary
        unmount_rootfs_if_needed(&rootfs_dir)?;

        std::fs::remove_dir_all(&rootfs_dir).ok();
        std::fs::create_dir_all(&rootfs_dir)?;

        let release = determine_release(&distro, release)?;

        // Install newer debootstrap if needed
        install_debootstrap_if_needed(&distro, &release)?;

        let mut build_args = HashMap::new();
        build_args.insert("custom_packages".to_string(), desktop_package);
        build_args.insert("hostname".to_string(), format!("shimboot-{}", board));
        build_args.insert("username".to_string(), "user".to_string());
        build_args.insert("user_passwd".to_string(), "user".to_string());
        build_args.insert("arch".to_string(), arch.clone());
        build_args.insert("distro".to_string(), distro.clone());

        crate::build_rootfs::run(
            rootfs_dir.to_str().unwrap(),
            &release,
            build_args,
        )?;

        rootfs_dir
    };

    // Patch rootfs
    print_title(&format!("patching {} rootfs", distro));
    let mut patch_args = HashMap::new();
    if quiet {
        patch_args.insert("quiet".to_string(), "1".to_string());
    }
    
    retry_command(|| {
        crate::patch_rootfs::run(
            shim_bin.to_str().unwrap(),
            reco_bin.to_str().unwrap(),
            rootfs_dir.to_str().unwrap(),
            patch_args.clone(),
        )
    })?;

    // Build final image
    print_title("building final disk image");
    let final_image = data_dir.join(format!("shimboot_{}.bin", board));
    std::fs::remove_file(&final_image).ok();

    let mut build_args = HashMap::new();
    if quiet {
        build_args.insert("quiet".to_string(), "1".to_string());
    }
    build_args.insert("arch".to_string(), arch);
    build_args.insert("name".to_string(), distro.clone());
    if luks {
        build_args.insert("luks".to_string(), "1".to_string());
    }

    retry_command(|| {
        crate::build::run(
            final_image.to_str().unwrap(),
            shim_bin.to_str().unwrap(),
            rootfs_dir.to_str().unwrap(),
            build_args.clone(),
        )
    })?;

    print_info(&format!("build complete! the final disk image is located at {}", final_image.display()));

    print_title("cleaning up");
    crate::image_utils::clean_loops()?;

    if compress_img {
        print_title("compressing disk image into a zip file");
        let image_zip = data_dir.join(format!("shimboot_{}.zip", board));
        run_command_inherit("zip", &["-j", image_zip.to_str().unwrap(), final_image.to_str().unwrap()])?;
        print_info("finished compressing the disk file");
        print_info(&format!("the finished zip file can be found at {}", image_zip.display()));
    }

    Ok(())
}

fn find_recovery_url(boards: &serde_json::Value, board: &str) -> Result<String> {
    let builds = boards["builds"]
        .get(board)
        .context("Invalid board name")?;

    let mut board_data = builds;
    if let Some(models) = builds.get("models") {
        for device in models.as_object().unwrap().values() {
            if device.get("pushRecoveries").is_some() {
                board_data = device;
                break;
            }
        }
    }

    let recoveries = board_data["pushRecoveries"]
        .as_object()
        .context("No recovery images found")?;

    let url = recoveries
        .values()
        .last()
        .and_then(|v| v.as_str())
        .context("Could not find recovery URL")?;

    Ok(url.to_string())
}

fn download_and_unzip(url: &str, zip_path: &Path, bin_path: &Path, quiet: bool) -> Result<()> {
    if !bin_path.exists() {
        if !zip_path.exists() {
            let args = if quiet {
                vec!["-q", url, "-O", zip_path.to_str().unwrap(), "-c"]
            } else {
                vec!["-q", "--show-progress", url, "-O", zip_path.to_str().unwrap(), "-c"]
            };
            run_command_inherit("wget", &args)?;
        }

        extract_zip(zip_path, bin_path, quiet)?;
    }

    Ok(())
}

fn extract_zip(zip_path: &Path, bin_path: &Path, quiet: bool) -> Result<()> {
    print_info(&format!("extracting {}", zip_path.display()));

    if quiet {
        run_command_inherit("unzip", &["-p", zip_path.to_str().unwrap()])?;
        std::fs::rename("tmp_extract", bin_path)?;
    } else {
        // Get total size
        let output = std::process::Command::new("unzip")
            .args(["-lq", zip_path.to_str().unwrap()])
            .output()?;
        let listing = String::from_utf8_lossy(&output.stdout);
        let total_bytes = listing
            .lines()
            .last()
            .and_then(|l| l.split_whitespace().next())
            .unwrap_or("0");

        // Extract with pv
        let unzip = std::process::Command::new("unzip")
            .args(["-p", zip_path.to_str().unwrap()])
            .stdout(std::process::Stdio::piped())
            .spawn()?;

        let mut pv = std::process::Command::new("pv")
            .args(["-s", total_bytes])
            .stdin(unzip.stdout.unwrap())
            .stdout(std::process::Stdio::piped())
            .spawn()?;

        let mut file = std::fs::File::create(bin_path)?;
        std::io::copy(&mut pv.stdout.as_mut().unwrap(), &mut file)?;
    }

    std::fs::remove_file(zip_path)?;

    Ok(())
}

fn download_shim(board: &str, data_dir: &Path, shim_bin: &Path, quiet: bool) -> Result<()> {
    print_info("downloading shim file manifest");
    
    let boards_index = reqwest::blocking::get("https://cdn.cros.download/boards.txt")?
        .text()?;

    let shim_url_path = boards_index
        .lines()
        .find(|l| l.contains(&format!("/{}/", board)))
        .map(|l| format!("{}.manifest", l))
        .context("Board not found in shim index")?;

    let manifest_url = format!("https://cdn.cros.download/{}", shim_url_path);
    let shim_manifest = reqwest::blocking::get(&manifest_url)?.text()?;
    let manifest: serde_json::Value = serde_json::from_str(&shim_manifest)?;

    let _zip_size = manifest["size"].as_u64().unwrap();
    let chunks: Vec<String> = manifest["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();

    let shim_dir = data_dir.join(format!("shim_{}_chunks", board));
    std::fs::create_dir_all(&shim_dir)?;

    print_info(&format!("downloading shim file chunks (total {} chunks)", chunks.len()));

    let shim_url_dir = std::path::Path::new(&shim_url_path)
        .parent()
        .unwrap()
        .to_str()
        .unwrap();

    for (i, chunk) in chunks.iter().enumerate() {
        let chunk_url = format!("https://cdn.cros.download/{}/{}", shim_url_dir, chunk);
        let chunk_path = shim_dir.join(chunk);

        if chunk_path.exists() {
            continue;
        }

        print_info(&format!("downloading chunk {} / {}", i + 1, chunks.len()));
        let args = if quiet {
            vec!["-c", "-q", &chunk_url, "-O", chunk_path.to_str().unwrap()]
        } else {
            vec!["-c", "-q", "--show-progress", &chunk_url, "-O", chunk_path.to_str().unwrap()]
        };
        run_command_inherit("wget", &args)?;
    }

    print_info("joining shim file chunks");
    let shim_zip = data_dir.join(format!("shim_{}.zip", board));

    if !shim_bin.exists() {
        // Concatenate chunks
        let mut output = std::fs::File::create(&shim_zip)?;
        for chunk in &chunks {
            let chunk_path = shim_dir.join(chunk);
            let mut chunk_file = std::fs::File::open(&chunk_path)?;
            std::io::copy(&mut chunk_file, &mut output)?;
        }
        std::fs::remove_dir_all(&shim_dir)?;

        print_info("extracting shim file");
        extract_zip(&shim_zip, shim_bin, quiet)?;
    }

    Ok(())
}

fn unmount_rootfs_if_needed(rootfs_dir: &Path) -> Result<()> {
    let dev_path = rootfs_dir.join("dev");
    let output = std::process::Command::new("findmnt")
        .args(["-T", dev_path.to_str().unwrap()])
        .output();

    if let Ok(out) = output {
        if !out.stdout.is_empty() {
            let _ = run_command_inherit("umount", &["-l", &format!("{}/*", rootfs_dir.display())]);
        }
    }

    Ok(())
}

fn determine_release(distro: &str, release: Option<String>) -> Result<String> {
    if let Some(r) = release {
        return Ok(r);
    }

    match distro {
        "debian" => Ok("bookworm".to_string()),
        "ubuntu" => Ok("noble".to_string()),
        "alpine" => Ok("edge".to_string()),
        _ => anyhow::bail!("invalid distro selection"),
    }
}

fn install_debootstrap_if_needed(distro: &str, release: &str) -> Result<()> {
    if !Path::new("/etc/debian_version").exists() {
        return Ok(());
    }

    if distro != "ubuntu" && distro != "debian" {
        return Ok(());
    }

    let script_path = format!("/usr/share/debootstrap/scripts/{}", release);
    if Path::new(&script_path).exists() {
        return Ok(());
    }

    print_info("installing newer debootstrap version");
    let mirror_url = "https://deb.debian.org/debian/pool/main/d/debootstrap/";
    let listing = reqwest::blocking::get(mirror_url)?.text()?;
    
    let re = regex::Regex::new(r#"href="(debootstrap_.+?\.deb)""#)?;
    let deb_file = re.captures_iter(&listing)
        .last()
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
        .context("Could not find debootstrap package")?;

    let deb_url = format!("{}{}", mirror_url, deb_file);
    let deb_path = format!("/tmp/{}", deb_file);

    run_command_inherit("wget", &["-q", "--show-progress", &deb_url, "-O", &deb_path])?;
    run_command_inherit("apt-get", &["install", "-y", &deb_path])?;

    Ok(())
}

fn retry_command<F>(mut f: F) -> Result<()>
where
    F: FnMut() -> Result<()>,
{
    for _ in 0..5 {
        if f().is_ok() {
            return Ok(());
        }
    }
    f()
}
