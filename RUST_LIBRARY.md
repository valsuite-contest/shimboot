# Shimboot Shim Manager - Rust Library

A Rust library for managing and editing Chromebook RMA shims. This library provides a safe, high-level API for working with Chrome OS shim images, extracting and modifying initramfs contents, and injecting custom payloads.

## Overview

This library allows you to:

- **Extract initramfs** from Chrome OS kernel images
- **Modify initramfs contents** programmatically
- **Inject custom payloads** into the boot process
- **Build modified initramfs** with custom scripts and files

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
shimboot-shim = "0.1.0"
```

## Quick Start

### Basic Usage

```rust
use shimboot_shim::{Shim, InitramfsBuilder};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open a shim image
    let shim = Shim::open(Path::new("shim.bin"))?;
    
    // Extract the initramfs
    let initramfs = shim.extract_initramfs()?;
    
    // Create a modified initramfs
    let mut builder = InitramfsBuilder::from_initramfs(initramfs)?;
    
    // Add a custom payload
    let payload = b"#!/bin/sh\necho 'Hello from custom payload!'\n";
    builder.add_payload_script("custom.sh", payload)?;
    
    // Build and save
    let modified = builder.build()?;
    modified.save(Path::new("modified_initramfs"))?;
    
    Ok(())
}
```

### Creating a Custom Welcome Message

```rust
use shimboot_shim::InitramfsBuilder;

fn create_welcome_shim() -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = InitramfsBuilder::new();
    
    // Create a welcome payload
    let welcome_script = r#"#!/bin/busybox sh
cat << 'EOF'
╔═══════════════════════════════════╗
║   Welcome to Shimboot!            ║
║   Custom Payload Active           ║
╚═══════════════════════════════════╝
EOF
sleep 2
"#;
    
    // Add the payload script
    builder.add_payload_script("welcome.sh", welcome_script.as_bytes())?;
    
    // Modify the init script to call our payload
    let init_script = create_custom_init(); // Your custom init
    builder.replace_init(&init_script)?;
    
    // Build the initramfs
    let initramfs = builder.build()?;
    initramfs.save(Path::new("/tmp/custom_initramfs"))?;
    
    Ok(())
}
```

## API Documentation

### `Shim`

Represents a Chrome OS RMA shim image.

```rust
pub struct Shim {
    // ...
}

impl Shim {
    /// Open a shim image file
    pub fn open(path: &Path) -> Result<Self>;
    
    /// Extract the initramfs from the shim's kernel
    pub fn extract_initramfs(&self) -> Result<Initramfs>;
}
```

### `Initramfs`

Represents an initramfs (initial RAM filesystem).

```rust
pub struct Initramfs {
    pub files: HashMap<String, Vec<u8>>,
    pub metadata: HashMap<String, FileMetadata>,
}

impl Initramfs {
    /// Create a new empty initramfs
    pub fn new() -> Self;
    
    /// Get the content of a file
    pub fn get_file(&self, path: &str) -> Option<&[u8]>;
    
    /// Add or update a file
    pub fn add_file(&mut self, path: String, content: Vec<u8>, metadata: FileMetadata);
    
    /// Remove a file
    pub fn remove_file(&mut self, path: &str) -> Option<Vec<u8>>;
    
    /// Save the initramfs to a directory
    pub fn save(&self, dir: &Path) -> Result<()>;
}
```

### `InitramfsBuilder`

Builder for creating modified initramfs.

```rust
pub struct InitramfsBuilder {
    // ...
}

impl InitramfsBuilder {
    /// Create a new builder from an existing initramfs
    pub fn from_initramfs(initramfs: Initramfs) -> Result<Self>;
    
    /// Create a new empty builder
    pub fn new() -> Self;
    
    /// Add a file with default permissions (0644)
    pub fn add_file(&mut self, path: &str, content: &[u8]) -> Result<&mut Self>;
    
    /// Add an executable file (0755)
    pub fn add_executable(&mut self, path: &str, content: &[u8]) -> Result<&mut Self>;
    
    /// Add a file with specific permissions
    pub fn add_file_with_mode(&mut self, path: &str, content: &[u8], mode: u32) -> Result<&mut Self>;
    
    /// Remove a file from the initramfs
    pub fn remove_file(&mut self, path: &str) -> Result<&mut Self>;
    
    /// Replace the init script with a custom one
    pub fn replace_init(&mut self, init_script: &[u8]) -> Result<&mut Self>;
    
    /// Add a custom payload script to bin/
    pub fn add_payload_script(&mut self, name: &str, script: &[u8]) -> Result<&mut Self>;
    
    /// Build the final initramfs
    pub fn build(self) -> Result<Initramfs>;
}
```

## Examples

The `examples/` directory contains complete examples:

### Welcome Payload Example

Run the example to see how to create a custom payload:

```bash
cargo run --example welcome_payload
```

This example demonstrates:
1. Creating a custom welcome message payload
2. Injecting it into the bootloader
3. Modifying the init script to execute the payload
4. Building and saving the modified initramfs

The output is saved to `/tmp/shimboot_custom_initramfs/` for inspection.

## How It Works

### Initramfs Extraction Process

1. **Find gzip compressed data**: Scans the kernel for gzip magic bytes (0x1f 0x8b)
2. **Decompress kernel**: Uses flate2 to decompress the gzip data
3. **Find CPIO archive**: Searches for CPIO magic bytes ("070701" or "070702")
4. **Parse CPIO**: Reads the CPIO newc format archive
5. **Extract files**: Builds an in-memory representation of all files

### Custom Payload Injection

Custom payloads can be injected at several points:

1. **In `/init`**: The first script executed by the kernel
2. **In `bin/bootstrap.sh`**: The main bootloader logic
3. **As separate scripts**: Called from existing scripts

### File Permissions

The library preserves and allows setting Unix file permissions:

```rust
builder.add_file_with_mode("bin/script.sh", script, 0o755)?;
```

On Unix systems, these permissions are applied when saving to disk.

## Architecture

```
Shim Image (disk image)
├── Partition 1: Stateful (1MB)
├── Partition 2: Kernel (32MB)
│   ├── Kernel header
│   ├── Compressed kernel (gzip)
│   │   └── Initramfs (CPIO archive)  ← This library works here
│   └── Padding
├── Partition 3: Bootloader files (20MB)
└── Partition 4: Root filesystem
```

The library focuses on extracting and modifying the initramfs within Partition 2.

## Limitations

- **Partition parsing**: Currently simplified; full GPT parsing could be added
- **ARM support**: LZ4 decompression for ARM kernels not yet implemented
- **CPIO writing**: Currently only reads CPIO; writing back to CPIO format is TODO
- **Kernel repacking**: Full kernel image rebuilding not yet implemented

## Complete Workflow

For a complete shim modification workflow:

1. **Extract kernel** from shim image (partition 2)
2. **Use this library** to extract and modify initramfs
3. **Repack initramfs** into CPIO format
4. **Rebuild kernel** with modified initramfs
5. **Write kernel** back to partition 2 of shim image

Steps 3-5 currently require external tools (binwalk, cpio, etc.) but could be implemented in this library in the future.

## Contributing

Contributions are welcome! Areas for improvement:

- Full GPT partition table parsing
- ARM64 LZ4 kernel decompression
- CPIO archive creation/writing
- Complete kernel repacking
- More examples and documentation

## License

This library is part of the Shimboot project and is licensed under the GNU General Public License v3.0.

See the [LICENSE](../LICENSE) file for details.

## See Also

- [Shimboot Main Repository](https://github.com/ading2210/shimboot)
- [How It Works](../HOW_IT_WORKS.md) - Detailed technical explanation
- [sh1mmer.me](https://sh1mmer.me/) - Information about the security vulnerability
