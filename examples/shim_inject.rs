//! Standalone example: Inject "Welcome to Shimboot" payload into a real shim
//!
//! This example demonstrates a complete, standalone workflow:
//! 1. Extract initramfs from a Chrome OS RMA shim
//! 2. Inject a custom welcome payload
//! 3. Save the modified initramfs
//!
//! Usage:
//!   cargo run --example shim_inject -- <shim_file.bin> <output_dir>
//!
//! Example:
//!   cargo run --example shim_inject -- /path/to/shim-octopus.bin ./modified_shim
//!
//! This example is fully standalone and can be moved outside the repository.
//! It does not depend on any bash scripts from the shimboot project.

use shimboot_shim::{Shim, InitramfsBuilder, Result};
use std::env;
use std::path::Path;
use std::process;

fn main() -> Result<()> {
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    
    if args.len() != 3 {
        eprintln!("Usage: {} <shim_file.bin> <output_dir>", args[0]);
        eprintln!();
        eprintln!("Example:");
        eprintln!("  {} shim-octopus.bin ./modified_shim", args[0]);
        eprintln!();
        eprintln!("This will:");
        eprintln!("  1. Extract the initramfs from the shim");
        eprintln!("  2. Inject a custom 'Welcome to Shimboot' payload");
        eprintln!("  3. Save the modified initramfs to the output directory");
        process::exit(1);
    }
    
    let shim_path = Path::new(&args[1]);
    let output_dir = Path::new(&args[2]);
    
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║  Shimboot Shim Payload Injector - Standalone Example      ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();
    
    // Step 1: Open and verify the shim file
    println!("[1/5] Opening shim file: {}", shim_path.display());
    let shim = Shim::open(shim_path)?;
    println!("      ✓ Shim file opened successfully");
    println!();
    
    // Step 2: Extract the initramfs
    println!("[2/5] Extracting initramfs from shim...");
    println!("      This may take a moment as we scan for the kernel and CPIO archive");
    let initramfs = shim.extract_initramfs()?;
    println!("      ✓ Extracted {} files from initramfs", initramfs.files.len());
    println!();
    
    // Step 3: Create the payload and modify initramfs
    println!("[3/5] Injecting custom welcome payload...");
    let mut builder = InitramfsBuilder::from_initramfs(initramfs)?;
    
    // Add the welcome payload script
    let welcome_payload = create_welcome_payload();
    builder.add_payload_script("welcome_payload.sh", &welcome_payload)?;
    println!("      ✓ Added welcome_payload.sh");
    
    // Modify the init script to call our payload
    let init_script = create_init_with_payload();
    builder.replace_init(&init_script)?;
    println!("      ✓ Modified init script to execute payload");
    
    // Optionally modify bootstrap.sh
    let bootstrap_script = create_bootstrap_with_payload();
    builder.add_file_with_mode("bin/bootstrap.sh", &bootstrap_script, 0o755)?;
    println!("      ✓ Enhanced bootstrap.sh with welcome message");
    println!();
    
    // Step 4: Build the modified initramfs
    println!("[4/5] Building modified initramfs...");
    let modified_initramfs = builder.build()?;
    println!("      ✓ Built initramfs with {} files", modified_initramfs.files.len());
    println!();
    
    // Step 5: Save the modified initramfs
    println!("[5/5] Saving modified initramfs to: {}", output_dir.display());
    modified_initramfs.save(output_dir)?;
    println!("      ✓ Saved successfully");
    println!();
    
    // Print success summary
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║  SUCCESS! Payload injected successfully                   ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();
    println!("Modified initramfs saved to: {}", output_dir.display());
    println!();
    println!("What's been added:");
    println!("  • bin/welcome_payload.sh - Custom welcome script");
    println!("  • Modified init - Calls welcome payload on boot");
    println!("  • Enhanced bootstrap.sh - Shows welcome banner");
    println!();
    println!("When this shim boots, it will display:");
    println!();
    println!("  ╔═══════════════════════════════════╗");
    println!("  ║   Welcome to Shimboot!            ║");
    println!("  ║   Custom Payload Active           ║");
    println!("  ╚═══════════════════════════════════╝");
    println!();
    println!("Next steps to use this modified initramfs:");
    println!("  1. Repack the initramfs into a CPIO archive");
    println!("  2. Inject it back into the kernel image");
    println!("  3. Write the modified kernel to the shim's partition 2");
    println!();
    println!("Note: Steps 1-3 require external tools like cpio and dd.");
    println!("      See HOW_IT_WORKS.md for detailed instructions.");
    
    Ok(())
}

/// Create the welcome payload script that will execute on boot
fn create_welcome_payload() -> Vec<u8> {
    let script = r#"#!/bin/busybox sh
# Custom Shimboot Welcome Payload
# This script runs early in the boot process

# Display a welcome banner
cat << 'EOF'
╔═══════════════════════════════════╗
║   Welcome to Shimboot!            ║
║   Custom Payload Active           ║
╚═══════════════════════════════════╝
EOF

# Show boot information
echo "Custom payload executed successfully"
echo "Boot time: $(date)"
echo ""

# Wait briefly so user can see the message
sleep 2
"#;
    script.as_bytes().to_vec()
}

/// Create a modified init script that calls our custom payload
fn create_init_with_payload() -> Vec<u8> {
    let script = r#"#!/bin/busybox sh
# Copyright 2015 The Chromium OS Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.
#
# Modified init script with custom payload injection

set -x

detect_tty() {
  if [ -f "/bin/frecon-lite" ]; then
    export TTY1="/dev/pts/0"
    export TTY2="/dev/pts/1"
  else
    export TTY1="/dev/tty1"
    export TTY2="/dev/tty2"
  fi
}

setup_environment() {
  # Install additional utility programs.
  /bin/busybox --install /bin || true
}

run_custom_payload() {
  # Execute our custom welcome payload
  if [ -x "/bin/welcome_payload.sh" ]; then
    /bin/welcome_payload.sh
  fi
}

main() {
  setup_environment
  detect_tty
  
  # Run the custom payload before starting bootstrap
  run_custom_payload
  
  # In case an error is not handled by bootstrapping, stop here
  # so that an operator can see installation stop.
  exec bootstrap.sh < "$TTY1" >> "$TTY1" 2>&1 || sleep 1d
}

main "$@"
exit 1
"#;
    script.as_bytes().to_vec()
}

/// Create an enhanced bootstrap.sh with welcome banner
fn create_bootstrap_with_payload() -> Vec<u8> {
    let script = r#"#!/bin/busybox sh
# Copyright 2015 The Chromium OS Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.
#
# Bootstrap script enhanced with custom welcome payload

set +x

rescue_mode=""

invoke_terminal() {
  local tty="$1"
  local title="$2"
  shift
  shift
  echo "${title}" >>${tty}
  setsid sh -c "exec script -afqc '$*' /dev/null <${tty} >>${tty} 2>&1 &"
}

enable_debug_console() {
  local tty="$1"
  echo -e "debug console enabled on ${tty}"
  invoke_terminal "${tty}" "[Bootstrap Debug Console]" "/bin/busybox sh"
}

get_part_dev() {
  local disk="$1"
  local partition="$2"
  last_char="$(echo -n "$disk" | tail -c 1)"
  if [ "$last_char" -eq "$last_char" ] 2>/dev/null; then
    echo "${disk}p${partition}"
  else
    echo "${disk}${partition}"
  fi
}

find_rootfs_partitions() {
  local disks=$(fdisk -l | sed -n "s/Disk \(\/dev\/.*\):.*/\1/p")
  if [ ! "${disks}" ]; then
    return 1
  fi
  for disk in $disks; do
    local partitions=$(fdisk -l $disk | sed -n "s/^[ ]\+\([0-9]\+\).*shimboot_rootfs:\(.*\)$/\1:\2/p")
    if [ ! "${partitions}" ]; then
      continue
    fi
    for partition in $partitions; do
      get_part_dev "$disk" "$partition"
    done
  done
}

find_chromeos_partitions() {
  local roota_partitions="$(cgpt find -l ROOT-A)"
  local rootb_partitions="$(cgpt find -l ROOT-B)"
  if [ "$roota_partitions" ]; then
    for partition in $roota_partitions; do
      echo "${partition}:ChromeOS_ROOT-A:CrOS"
    done
  fi
  if [ "$rootb_partitions" ]; then
    for partition in $rootb_partitions; do
      echo "${partition}:ChromeOS_ROOT-B:CrOS"
    done
  fi
}

find_all_partitions() {
  echo "$(find_chromeos_partitions)"
  echo "$(find_rootfs_partitions)"
}

move_mounts() {
  local base_mounts="/sys /proc /dev"
  local newroot_mnt="$1"
  for mnt in $base_mounts; do
    mkdir -p "$newroot_mnt$mnt"
    mount -n -o move "$mnt" "$newroot_mnt$mnt"
  done
}

print_license() {
  local shimboot_version="$(cat /opt/.shimboot_version 2>/dev/null || echo 'custom')"
  if [ -f "/opt/.shimboot_version_dev" ]; then
    local git_hash="$(cat /opt/.shimboot_version_dev)"
    local suffix="-dev-$git_hash"
  fi
  cat << EOF 
Shimboot ${shimboot_version}${suffix}

ading2210/shimboot: Boot desktop Linux from a Chrome OS RMA shim.
Copyright (C) 2025 ading2210

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.
EOF
}

print_welcome() {
  cat << 'EOF'

  ╔═══════════════════════════════════╗
  ║   Welcome to Shimboot!            ║
  ║   Custom Payload Active           ║
  ╚═══════════════════════════════════╝

EOF
}

print_selector() {
  local rootfs_partitions="$1"
  local i=1

  echo "┌──────────────────────┐"
  echo "│ Shimboot OS Selector │"
  echo "└──────────────────────┘"

  if [ "${rootfs_partitions}" ]; then
    for rootfs_partition in $rootfs_partitions; do
      local part_path=$(echo $rootfs_partition | cut -d ":" -f 1)
      local part_name=$(echo $rootfs_partition | cut -d ":" -f 2)
      echo "${i}) ${part_name} on ${part_path}"
      i=$((i+1))
    done
  else
    echo "no bootable partitions found. please see the shimboot documentation to mark a partition as bootable."
  fi

  echo "q) reboot"
  echo "s) enter a shell"
  echo "l) view license"
}

get_selection() {
  local rootfs_partitions="$1"
  local i=1

  read -p "Your selection: " selection
  if [ "$selection" = "q" ]; then
    echo "rebooting now."
    reboot -f
  elif [ "$selection" = "s" ]; then
    reset
    enable_debug_console "$TTY1"
    return 0
  elif [ "$selection" = "l" ]; then
    clear
    print_license
    echo
    read -p "press [enter] to return to the bootloader menu"
    return 1
  fi

  local selection_cmd="$(echo "$selection" | cut -d' ' -f1)"
  if [ "$selection_cmd" = "rescue" ]; then
    selection="$(echo "$selection" | cut -d' ' -f2-)"
    rescue_mode="1"
  else
    rescue_mode=""
  fi

  for rootfs_partition in $rootfs_partitions; do
    local part_path=$(echo $rootfs_partition | cut -d ":" -f 1)
    local part_name=$(echo $rootfs_partition | cut -d ":" -f 2)
    local part_flags=$(echo $rootfs_partition | cut -d ":" -f 3)

    if [ "$selection" = "$i" ]; then
      echo "selected $part_path"
      if [ "$part_flags" = "CrOS" ]; then
        echo "booting chrome os partition"
        print_donor_selector "$rootfs_partitions"
        get_donor_selection "$rootfs_partitions" "$part_path"
      else
        boot_target "$part_path"
      fi
      return 1
    fi

    i=$((i+1))
  done
  
  echo "invalid selection"
  sleep 1
  return 1
}

exec_init() {
  if [ "$rescue_mode" = "1" ]; then
    echo "entering a rescue shell instead of starting init"
    echo "once you are done fixing whatever is broken, run 'exec /sbin/init' to continue booting the system normally"
    if [ -f "/bin/bash" ]; then
      exec /bin/bash < "$TTY1" >> "$TTY1" 2>&1
    else
      exec /bin/sh < "$TTY1" >> "$TTY1" 2>&1
    fi
  else
    exec /sbin/init < "$TTY1" >> "$TTY1" 2>&1
  fi
}

boot_target() {
  local target="$1"
  echo "moving mounts to newroot"
  mkdir /newroot
  if [ -x "$(command -v cryptsetup)" ] && cryptsetup luksDump "$target" >/dev/null 2>&1; then
    cryptsetup open $target rootfs
    mount /dev/mapper/rootfs /newroot
  else
    mount $target /newroot
  fi
  if [ -f "/bin/frecon-lite" ]; then 
    rm -f /dev/console
    touch /dev/console
    mount -o bind "$TTY1" /dev/console
  fi
  move_mounts /newroot
  echo "switching root"
  mkdir -p /newroot/bootloader
  pivot_root /newroot /newroot/bootloader
  exec_init
}

main() {
  echo "starting the shimboot bootloader"
  
  # Display the custom welcome banner
  print_welcome
  
  enable_debug_console "$TTY2"

  local valid_partitions="$(find_all_partitions)"

  while true; do
    clear
    print_welcome
    print_selector "${valid_partitions}"

    if get_selection "${valid_partitions}"; then
      break
    fi
  done
}

trap - EXIT
main "$@"
sleep 1d
"#;
    script.as_bytes().to_vec()
}
