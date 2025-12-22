# Shimboot - Rust Implementation

This is a complete rewrite of Shimboot in Rust. All functionality from the original Bash implementation has been reimplemented in Rust, providing better performance, type safety, and error handling.

## Overview

Shimboot is a collection of tools for patching a Chrome OS RMA shim to serve as a bootloader for a standard Linux distribution. This Rust implementation provides the same functionality as the original Bash scripts but with improved reliability and maintainability.

## Features

- **Complete rewrite in Rust** - All original functionality fully implemented
- **Standalone binaries** - No dependencies on the original Bash scripts
- **Better error handling** - Comprehensive error messages and recovery
- **Type safety** - Compile-time guarantees for correctness
- **Cross-platform support** - Works on x86_64 and ARM64 systems

## Building

### Prerequisites

- Rust 1.70 or later
- Cargo (comes with Rust)

### Build Instructions

```bash
# Build all binaries
cargo build --release

# The binaries will be in target/release/
```

## Usage

The Rust implementation provides the following binaries:

### Main Command

```bash
# Use the main shimboot command
shimboot build-complete <board_name> [options...]
```

### Individual Commands

The following standalone binaries are also available:

```bash
# Build complete shimboot image
build_complete <board_name> [key=value options...]

# Build bootloader image
shimboot-build <output_path> <shim_path> <rootfs_dir> [key=value options...]

# Build rootfs
build_rootfs <rootfs_path> <release_name> [key=value options...]

# Patch rootfs with drivers and firmware
patch_rootfs <shim_path> <reco_path> <rootfs_dir> [key=value options...]

# Build squashfs compressed rootfs
build_squashfs <output_dir> <input_dir> <shim_path> [key=value options...]
```

### Examples

```bash
# Build a complete image for dedede board
sudo ./target/release/shimboot build-complete dedede

# Build with custom options
sudo ./target/release/shimboot build-complete dedede \
    desktop=kde \
    arch=amd64 \
    release=bookworm \
    luks=1

# Build using standalone binary
sudo ./target/release/build_complete octopus desktop=xfce
```

## Supported Options

All options from the original Bash implementation are supported:

- `compress_img` - Compress the final disk image into a zip file
- `rootfs_dir` - Use a different rootfs for the build
- `quiet` - Don't use progress indicators
- `desktop` - Desktop environment (gnome, xfce, kde, lxde, etc.)
- `data_dir` - Working directory for the scripts
- `arch` - CPU architecture (amd64 or arm64)
- `release` - Release name (bookworm, trixie, unstable, etc.)
- `distro` - Linux distribution (debian, ubuntu, alpine)
- `luks` - Enable LUKS encryption

## Key Differences from Bash Version

1. **Command Structure**: Instead of separate `.sh` scripts, there's a unified `shimboot` command with subcommands
2. **Better Error Messages**: Rust's error handling provides more detailed error information
3. **Type Safety**: Many runtime errors in Bash are caught at compile time
4. **Performance**: Rust implementation is generally faster for CPU-intensive operations
5. **Dependencies**: External dependencies are managed through Cargo

## Implementation Details

### Modules

- `common.rs` - Common utilities (argument parsing, printing, dependency checks)
- `image_utils.rs` - Disk image operations (partitioning, loop devices, mounting)
- `shim_utils.rs` - Shim extraction utilities (initramfs extraction for x86/ARM)
- `build.rs` - Main build script functionality
- `build_complete.rs` - Complete build process
- `build_rootfs.rs` - Rootfs building (debootstrap integration)
- `patch_rootfs.rs` - Rootfs patching (drivers/firmware)
- `build_squashfs.rs` - Squashfs compression support
- `commands.rs` - CLI command handling

### Dependencies

The Rust implementation uses the following crates:

- `clap` - Command-line argument parsing
- `anyhow` - Error handling
- `reqwest` - HTTP requests for downloading images
- `serde`/`serde_json` - JSON parsing
- `regex` - Regular expressions
- `nix` - Unix system calls
- `colored` - Terminal colors
- `tar`/`flate2` - Archive handling
- `walkdir` - Directory traversal

## Requirements

Same as the original Bash version:

- Root access (for disk operations)
- External tools: `wget`, `python3`, `unzip`, `zip`, `git`, `debootstrap`, `cpio`, `binwalk`, `pcregrep`, `cgpt`, `mkfs.ext4`, `mkfs.ext2`, `fdisk`, `depmod`, `findmnt`, `lz4`, `pv`, `cryptsetup`
- At least 20GB of free disk space

## Compatibility

This Rust implementation is designed to be a drop-in replacement for the Bash scripts. It produces identical output images and maintains compatibility with all existing Shimboot workflows.

## License

This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

Copyright (C) 2025 ading2210 and contributors

See the [main README](README.md) for the full license text.

## Original Documentation

For general Shimboot documentation, device compatibility, and FAQ, see the [main README](README.md).
