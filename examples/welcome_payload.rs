//! Example: Creating a custom payload that prints "welcome to shimboot"
//!
//! This example demonstrates how to use the shimboot-shim library to:
//! 1. Create a custom payload script
//! 2. Inject it into the bootloader
//! 3. Modify the bootstrap.sh to call the payload
//!
//! Usage:
//!   cargo run --example welcome_payload
//!
//! Note: This example creates a demonstration of the modified initramfs structure.
//! In practice, you would extract an actual shim, modify it, and rebuild the kernel.

use shimboot_shim::{InitramfsBuilder, Result};
use std::path::Path;

fn main() -> Result<()> {
    println!("Shimboot Custom Payload Example");
    println!("================================\n");

    // Create a new initramfs builder
    let mut builder = InitramfsBuilder::new();

    // Step 1: Create the custom payload script
    let welcome_payload = create_welcome_payload();
    println!("Created custom payload script:");
    println!("{}\n", String::from_utf8_lossy(&welcome_payload));

    // Step 2: Add the custom payload to the initramfs
    builder.add_payload_script("welcome_payload.sh", &welcome_payload)?;
    println!("✓ Added welcome_payload.sh to initramfs\n");

    // Step 3: Create a modified init script that calls our payload
    let init_script = create_init_with_payload();
    builder.replace_init(&init_script)?;
    println!("✓ Replaced init script to call payload\n");

    // Step 4: Create a modified bootstrap.sh
    let bootstrap_script = create_bootstrap_with_payload();
    builder.add_file_with_mode("bin/bootstrap.sh", &bootstrap_script, 0o755)?;
    println!("✓ Added modified bootstrap.sh\n");

    // Step 5: Add essential busybox symlink placeholder
    builder.add_file("bin/.busybox_placeholder", b"# Busybox will be installed here\n")?;

    // Step 6: Add version file
    builder.add_file("opt/.shimboot_version", b"0.1.0-custom\n")?;

    // Build the final initramfs
    let initramfs = builder.build()?;
    println!("✓ Built modified initramfs with {} files\n", initramfs.files.len());

    // Save the initramfs to a directory for inspection
    let output_dir = Path::new("/tmp/shimboot_custom_initramfs");
    initramfs.save(output_dir)?;
    println!("✓ Saved initramfs to: {}\n", output_dir.display());

    println!("Files in the custom initramfs:");
    let mut files: Vec<_> = initramfs.files.keys().collect();
    files.sort();
    for file in files {
        println!("  - {}", file);
    }

    println!("\n{}", "=".repeat(60));
    println!("SUCCESS!");
    println!("{}", "=".repeat(60));
    println!("\nThe custom initramfs has been created and saved to:");
    println!("  {}", output_dir.display());
    println!("\nTo use this in a real shim:");
    println!("  1. Extract a real Chrome OS RMA shim kernel");
    println!("  2. Extract its initramfs using binwalk");
    println!("  3. Merge this custom initramfs with the extracted one");
    println!("  4. Repack the cpio archive");
    println!("  5. Rebuild the kernel with the modified initramfs");
    println!("\nWhen booted, this shim will display:");
    println!("  ╔═══════════════════════════════════╗");
    println!("  ║   Welcome to Shimboot!            ║");
    println!("  ║   Custom Payload Active           ║");
    println!("  ╚═══════════════════════════════════╝");

    Ok(())
}

/// Create the welcome payload script
fn create_welcome_payload() -> Vec<u8> {
    let script = r#"#!/bin/busybox sh
# Custom Shimboot Payload
# This script runs during the boot process

# Display a welcome banner
cat << 'EOF'
╔═══════════════════════════════════╗
║   Welcome to Shimboot!            ║
║   Custom Payload Active           ║
╚═══════════════════════════════════╝
EOF

# Additional custom payload actions can go here
echo "Custom payload executed successfully"
echo "Boot time: $(date)"
echo ""

# Wait a moment so user can see the message
sleep 2
"#;
    script.as_bytes().to_vec()
}

/// Create an init script that calls our custom payload
fn create_init_with_payload() -> Vec<u8> {
    let script = r#"#!/bin/busybox sh
# Copyright 2015 The Chromium OS Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.
#
# Modified init script with custom payload

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

/// Create a bootstrap.sh script that also displays the welcome message
fn create_bootstrap_with_payload() -> Vec<u8> {
    let script = r#"#!/bin/busybox sh
# Copyright 2015 The Chromium OS Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.
#
# Modified bootstrap with custom payload integration

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

print_welcome() {
  cat << 'EOF'

╔═══════════════════════════════════╗
║   Welcome to Shimboot!            ║
║   Custom Payload Active           ║
╚═══════════════════════════════════╝

Custom bootloader loaded successfully.

EOF
}

print_license() {
  local shimboot_version="$(cat /opt/.shimboot_version 2>/dev/null || echo 'unknown')"
  cat << EOF 
Shimboot ${shimboot_version}

ading2210/shimboot: Boot desktop Linux from a Chrome OS RMA shim.
Copyright (C) 2025 ading2210

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.
EOF
}

print_selector() {
  echo "┌──────────────────────┐"
  echo "│ Shimboot OS Selector │"
  echo "└──────────────────────┘"
  echo ""
  echo "No partitions configured for this example."
  echo ""
  echo "q) quit (reboot)"
  echo "s) enter a shell"
  echo "l) view license"
}

get_selection() {
  read -p "Your selection: " selection
  
  if [ "$selection" = "q" ]; then
    echo "This is a demo - would reboot in a real system."
    return 0
  elif [ "$selection" = "s" ]; then
    echo "Entering shell..."
    enable_debug_console "$TTY1"
    return 0
  elif [ "$selection" = "l" ]; then
    clear
    print_license
    echo
    read -p "press [enter] to return to the bootloader menu"
    return 1
  fi
  
  echo "Invalid selection"
  sleep 1
  return 1
}

main() {
  echo "Starting the Shimboot bootloader..."
  echo ""
  
  # Display the custom welcome message
  print_welcome
  
  enable_debug_console "$TTY2"
  
  while true; do
    clear
    print_welcome
    print_selector
    
    if get_selection; then
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
