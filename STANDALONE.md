# Shimboot Shim - Standalone Payload Injector

This is a standalone Rust tool for injecting custom payloads into Chrome OS RMA shim files. It can be used completely independently from the main shimboot repository.

## What This Does

This tool allows you to:
1. Extract the initramfs from a Chrome OS RMA shim
2. Inject custom boot scripts and payloads
3. Save the modified initramfs ready for repacking

The example tool injects a "Welcome to Shimboot!" banner that displays during boot.

## Requirements

- Rust toolchain (install from https://rustup.rs/)
- A Chrome OS RMA shim file (e.g., `shim-octopus.bin`)

**That's it!** No other dependencies required. This tool is completely self-contained.

## Quick Start

### 1. Clone or copy this directory

If you're working from the shimboot repository:
```bash
# Just this directory and Cargo.toml/src/ are needed
mkdir my-shim-tool
cd my-shim-tool
# Copy Cargo.toml, src/, and examples/
```

Or create a new Rust project:
```bash
cargo new --lib shimboot-shim
cd shimboot-shim
# Copy the contents from this repository
```

### 2. Run the injection tool

```bash
cargo run --example shim_inject -- <shim_file.bin> <output_dir>
```

Example:
```bash
cargo run --example shim_inject -- shim-octopus.bin ./modified_shim
```

### 3. What you get

After running, you'll have a directory with the modified initramfs:
```
modified_shim/
├── init                        # Modified init script (calls payload)
├── bin/
│   ├── bootstrap.sh            # Enhanced bootloader
│   ├── welcome_payload.sh      # Your custom payload!
│   └── ... (other files from original shim)
└── ... (other directories)
```

## Usage Examples

### Basic usage

```bash
# Inject welcome payload into a shim
cargo run --example shim_inject -- shim-dedede.bin ./output
```

### Compile and use the binary directly

```bash
# Build a release binary
cargo build --release --example shim_inject

# Use it
./target/release/examples/shim_inject shim-octopus.bin ./modified
```

## What the Tool Does

1. **Extracts initramfs**: Scans the shim file for gzip-compressed kernel data, decompresses it, and extracts the CPIO initramfs archive
2. **Injects payload**: Adds a custom `welcome_payload.sh` script to `bin/`
3. **Modifies init**: Updates the init script to call your payload during boot
4. **Enhances bootstrap**: Updates bootstrap.sh to show welcome banner
5. **Saves output**: Writes all files to the output directory

## Customizing the Payload

To create your own custom payload, edit `examples/shim_inject.rs`:

```rust
fn create_welcome_payload() -> Vec<u8> {
    let script = r#"#!/bin/busybox sh
# Your custom payload here!
echo "My custom message"
# ... more commands ...
"#;
    script.as_bytes().to_vec()
}
```

## Using the Library Programmatically

You can also use this as a library in your own Rust programs:

```rust
use shimboot_shim::{Shim, InitramfsBuilder};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open shim and extract initramfs
    let shim = Shim::open(Path::new("shim.bin"))?;
    let initramfs = shim.extract_initramfs()?;
    
    // Modify it
    let mut builder = InitramfsBuilder::from_initramfs(initramfs)?;
    builder.add_payload_script("my_script.sh", b"#!/bin/sh\necho Hello\n")?;
    
    // Save
    let modified = builder.build()?;
    modified.save(Path::new("output"))?;
    
    Ok(())
}
```

See [RUST_LIBRARY.md](RUST_LIBRARY.md) for complete API documentation.

## Next Steps: Using the Modified Initramfs

After extracting and modifying the initramfs with this tool, you need to:

1. **Repack to CPIO**:
   ```bash
   cd modified_shim
   find . | cpio -o -H newc > ../custom_initramfs.cpio
   ```

2. **Compress**:
   ```bash
   gzip -c custom_initramfs.cpio > custom_initramfs.cpio.gz
   ```

3. **Inject into kernel**: Use the shimboot build scripts or manual kernel patching

4. **Write to shim**: Replace partition 2 of the shim with the modified kernel

See the main [shimboot documentation](https://github.com/ading2210/shimboot) for complete instructions on the full workflow.

## Features

- ✅ Fully standalone - no external dependencies
- ✅ Works on real Chrome OS RMA shims
- ✅ Pure Rust implementation
- ✅ Safe, high-level API
- ✅ Handles gzip decompression automatically
- ✅ Parses CPIO newc format
- ✅ Preserves file permissions
- ✅ Easy to customize

## Limitations

Current version:
- Reads entire shim file (doesn't parse GPT - relies on magic byte detection)
- Only extracts/modifies initramfs (doesn't repack to kernel/shim)
- x86_64 focus (ARM64 LZ4 decompression not yet implemented)

These are documented in the code and can be extended as needed.

## License

GPL-3.0 - Same as the shimboot project

## Credits

Part of the [shimboot project](https://github.com/ading2210/shimboot) by ading2210.
This Rust library and standalone tool created to provide a programmatic interface to shim manipulation.
