# How Shimboot Works

## Table of Contents
- [Overview](#overview)
- [Chrome OS RMA Shims](#chrome-os-rma-shims)
- [The Security Flaw](#the-security-flaw)
- [Boot Process](#boot-process)
- [Partition Layout](#partition-layout)
- [Custom Code Execution from Shims](#custom-code-execution-from-shims)
  - [Understanding the Initramfs](#understanding-the-initramfs)
  - [Patching the Bootloader](#patching-the-bootloader)
  - [Custom Payload Execution](#custom-payload-execution)
- [Technical Implementation Details](#technical-implementation-details)

## Overview

Shimboot exploits a security vulnerability in Chrome OS RMA (Return Merchandise Authorization) shims to boot full Linux distributions on Chromebooks without modifying the device firmware or unenrolling the device.

## Chrome OS RMA Shims

Chrome OS RMA shims are bootable disk images designed by Google to run diagnostic utilities on Chromebooks. These shims have several important characteristics:

1. **Factory Diagnostics**: They contain tools for testing hardware components
2. **Bypass Enrollment**: They work even on enterprise-enrolled devices
3. **Signed Kernel**: The kernel partition is cryptographically signed and verified by the firmware
4. **Unverified Rootfs**: The root filesystem is **not** verified - this is the vulnerability

## The Security Flaw

The Chrome OS firmware verifies the kernel partition signature but does not verify the integrity of the root filesystem (initramfs). This allows us to:

1. Extract the initramfs from the shim kernel
2. Modify the initramfs contents
3. Repackage the kernel with our modified initramfs
4. Boot the modified system

This vulnerability is documented at [sh1mmer.me](https://sh1mmer.me/).

## Boot Process

The shimboot boot process follows these stages:

### 1. Firmware Stage
```
Chromebook Firmware
    ↓
Verifies kernel signature (KERN-A partition)
    ↓
Loads kernel into memory
    ↓
Kernel unpacks initramfs
```

### 2. Initramfs Stage (Our Custom Bootloader)
```
/init (busybox shell script)
    ↓
setup_environment() - Install busybox utilities
    ↓
detect_tty() - Determine correct TTY device
    ↓
bootstrap.sh - Main bootloader logic
```

### 3. Bootloader Stage (bootstrap.sh)
```
Print license and version
    ↓
find_all_partitions() - Scan for bootable partitions
    ↓
Display OS selector menu
    ↓
User selects partition
    ↓
boot_target() or boot_chromeos()
    ↓
pivot_root to selected OS
    ↓
exec /sbin/init (start OS init system)
```

### 4. OS Boot Stage
```
systemd (patched for Chrome OS kernel)
    ↓
Full Debian/Linux environment
```

## Partition Layout

Shimboot images use the following partition structure:

| Partition | Size | Type | Purpose |
|-----------|------|------|---------|
| p1 | 1MB | ext4 | Dummy stateful partition (required by Chrome OS) |
| p2 | 32MB | ChromeOS Kernel | Signed kernel containing our initramfs |
| p3 | 20MB | ext2 | Bootloader files and utilities |
| p4 | Variable | ext4 | Target OS rootfs (named `shimboot_rootfs:<name>`) |

## Custom Code Execution from Shims

### Understanding the Initramfs

The initramfs (initial RAM filesystem) is a compressed cpio archive embedded within the kernel image. When the kernel boots, it:

1. Unpacks the initramfs into a tmpfs in RAM
2. Executes `/init` from the initramfs
3. The init script sets up the environment and mounts the real root filesystem

**Key Point**: Since the initramfs is not verified, we can modify `/init` and any other files in the initramfs to run custom code.

### Patching the Bootloader

Shimboot replaces the original factory shim's `/init` script with a custom bootloader. Here's how the patching works:

#### Step 1: Extract the Initramfs

```bash
# From shim_utils.sh
extract_initramfs() {
  local kernel_bin="$1"        # Kernel partition extracted from shim
  local working_dir="$2"       # Temporary directory
  local output_dir="$3"        # Where to extract initramfs
  
  # Use binwalk to find and extract gzip compressed data
  # Then extract the cpio archive from within
  # Finally unpack the cpio to get initramfs contents
}
```

The kernel image structure is:
```
Kernel Partition (32MB)
├── Kernel header and signature
├── Compressed kernel (gzip)
│   └── Compressed initramfs (cpio archive inside)
└── Padding
```

#### Step 2: Modify the Initramfs

```bash
# From image_utils.sh
patch_initramfs() {
  local initramfs_path="$1"
  
  # Remove original init script
  rm "${initramfs_path}/init" -f
  
  # Copy our custom bootloader files
  cp -r bootloader/* "${initramfs_path}/"
  
  # Make binaries executable
  find ${initramfs_path}/bin -name "*" -exec chmod +x {} \;
}
```

Our custom bootloader structure:
```
initramfs/
├── bin/
│   ├── init          # Our custom init script
│   └── bootstrap.sh  # Main bootloader logic
└── opt/
    ├── crossystem    # Chrome OS utility
    └── mount-encrypted
```

#### Step 3: Rebuild the Image

The modified initramfs is repacked into the kernel partition, which is then written to the disk image.

### Custom Payload Execution

To run custom code at boot, you can modify the initramfs in several ways:

#### Method 1: Modify `/init`

The `/init` script (located at `bootloader/bin/init`) is the first script executed. You can add custom code here:

```bash
#!/bin/busybox sh
# Add custom payload here, before or after setup
echo "Custom payload: Welcome to shimboot!"

# Then continue with normal boot process
setup_environment
detect_tty
exec bootstrap.sh < "$TTY1" >> "$TTY1" 2>&1 || sleep 1d
```

#### Method 2: Modify `bootstrap.sh`

The `bootstrap.sh` script (located at `bootloader/bin/bootstrap.sh`) runs the main bootloader logic. You can add custom code in the `main()` function:

```bash
main() {
  # Custom payload here
  echo "Welcome to shimboot" > "$TTY1"
  
  # Continue with bootloader
  echo "starting the shimboot bootloader"
  enable_debug_console "$TTY2"
  # ... rest of bootloader
}
```

#### Method 3: Add Custom Scripts

You can add entirely new scripts to `bootloader/bin/` or `bootloader/opt/` and call them from the existing scripts:

```bash
# Create bootloader/bin/custom_payload.sh
#!/bin/busybox sh
echo "Welcome to shimboot"
# ... more custom code

# Then call it from init or bootstrap.sh
/bin/custom_payload.sh
```

#### Method 4: Hook Into Boot Process

You can inject code at specific points in the boot process:

```bash
# In bootstrap.sh, before boot_target() is called
boot_target() {
  local target="$1"
  
  # Custom payload before mounting rootfs
  echo "Welcome to shimboot - Loading $target"
  
  # Continue with normal boot
  mkdir /newroot
  mount $target /newroot
  # ... rest of boot_target
}
```

### Example: Welcome Message Payload

Here's a complete example of adding a welcome message:

1. **Edit `bootloader/bin/bootstrap.sh`**, add at the start of `main()`:
```bash
main() {
  # Welcome payload
  cat << 'EOF'
╔═══════════════════════════════════╗
║   Welcome to Shimboot!            ║
║   Custom Bootloader Active        ║
╚═══════════════════════════════════╝
EOF
  sleep 2
  
  # Continue with normal bootloader
  echo "starting the shimboot bootloader"
  # ... rest of main()
}
```

2. **Rebuild the image** with your modified bootloader:
```bash
sudo ./build.sh image.bin path_to_shim data/rootfs
```

The custom payload will execute every time the shim boots.

## Technical Implementation Details

### Kernel Extraction Process

The kernel extraction uses `binwalk` to identify and extract compressed data:

1. **Find gzip compressed kernel**: Scans for gzip magic bytes
2. **Extract kernel image**: Decompresses the gzip data
3. **Find cpio archive**: Scans decompressed kernel for cpio magic bytes
4. **Extract initramfs**: Unpacks the cpio archive

For ARM systems, the process is similar but uses LZ4 compression instead of gzip.

### Partition Detection

The bootloader scans for partitions in two ways:

1. **Chrome OS partitions**: Uses `cgpt` to find ROOT-A and ROOT-B partitions
2. **Shimboot partitions**: Uses `fdisk` to find partitions named `shimboot_rootfs:*`

### Root Switching

The bootloader uses `pivot_root` to switch from the initramfs to the target OS:

```bash
mount $target /newroot           # Mount target OS
move_mounts /newroot             # Move /sys, /proc, /dev to new root
pivot_root /newroot /newroot/bootloader  # Switch root
exec /sbin/init                  # Start target OS init
```

This allows the initramfs bootloader to remain accessible at `/bootloader` if needed for debugging.

### Systemd Compatibility

Standard systemd expects certain kernel features that the Chrome OS kernel disables. Shimboot uses [patched systemd](https://github.com/ading2210/chromeos-systemd) that works around these limitations:

- Disabled swap support
- Disabled suspend support  
- Modified mount handling to avoid kernel complaints

### Limitations

Because we're using the Chrome OS kernel from the shim:

- **No audio** on some boards (kernel doesn't load audio drivers)
- **No suspend** (disabled by kernel)
- **No swap** (disabled by kernel)
- **Kernel version locked** to shim version

These limitations can only be overcome by using kexec to load a different kernel (currently not working).
