//! # Shimboot Shim Manager
//!
//! A Rust library for managing and editing Chromebook RMA shims.
//!
//! This library provides functionality to:
//! - Extract initramfs from Chrome OS kernel images
//! - Modify initramfs contents
//! - Inject custom payloads into shims
//! - Rebuild kernel images with modified initramfs
//!
//! ## Example
//!
//! ```no_run
//! use shimboot_shim::{Shim, InitramfsBuilder};
//! use std::path::Path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Open a shim image
//! let shim = Shim::open(Path::new("shim.bin"))?;
//!
//! // Extract the initramfs
//! let initramfs = shim.extract_initramfs()?;
//!
//! // Create a modified initramfs with a custom payload
//! let mut builder = InitramfsBuilder::from_initramfs(initramfs)?;
//! builder.add_file("bin/custom_payload.sh", b"#!/bin/sh\necho 'Hello from custom payload!'\n")?;
//!
//! // Build and save the modified initramfs
//! let modified = builder.build()?;
//! modified.save(Path::new("modified_initramfs"))?;
//! # Ok(())
//! # }
//! ```

use std::fs::{self, File};
use std::io::{self, Read, Write, Cursor};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use flate2::read::GzDecoder;
use thiserror::Error;

/// Errors that can occur when working with shims
#[derive(Error, Debug)]
pub enum ShimError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Failed to find gzip compressed data in kernel (searched {0} bytes)")]
    GzipNotFound(usize),

    #[error("Failed to find cpio archive in decompressed kernel (searched {0} bytes)")]
    CpioNotFound(usize),

    #[error("Invalid kernel format: {0}")]
    InvalidKernel(String),

    #[error("Invalid initramfs format: {0}")]
    InvalidInitramfs(String),

    #[error("CPIO error: {0}")]
    Cpio(String),

    #[error("Path error: {0}")]
    Path(String),
}

pub type Result<T> = std::result::Result<T, ShimError>;

/// Represents a Chrome OS RMA shim image
pub struct Shim {
    path: PathBuf,
}

impl Shim {
    /// Open a shim image file
    pub fn open(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(ShimError::Io(io::Error::new(
                io::ErrorKind::NotFound,
                "Shim file not found",
            )));
        }
        Ok(Shim {
            path: path.to_path_buf(),
        })
    }

    /// Extract the kernel partition from the shim
    /// 
    /// # Note
    /// 
    /// This is a simplified implementation that reads the entire shim file.
    /// In a production implementation, this should:
    /// 1. Parse the GPT partition table
    /// 2. Locate partition 2 (KERN-A)
    /// 3. Extract only that partition's data
    /// 
    /// The current implementation works because `extract_initramfs_from_kernel`
    /// searches for gzip magic bytes, which will find the kernel regardless of
    /// extra data before or after it. However, this is inefficient for large
    /// shim files.
    pub fn extract_kernel(&self) -> Result<Vec<u8>> {
        let mut file = File::open(&self.path)?;
        let mut kernel_data = Vec::new();
        
        // TODO: Implement proper GPT parsing to extract only partition 2
        // For now, read the whole file and rely on magic byte detection
        file.read_to_end(&mut kernel_data)?;
        
        Ok(kernel_data)
    }

    /// Extract the initramfs from the shim's kernel
    pub fn extract_initramfs(&self) -> Result<Initramfs> {
        let kernel_data = self.extract_kernel()?;
        extract_initramfs_from_kernel(&kernel_data)
    }
}

/// Represents an initramfs (initial RAM filesystem)
#[derive(Debug)]
pub struct Initramfs {
    pub files: HashMap<String, Vec<u8>>,
    pub metadata: HashMap<String, FileMetadata>,
}

/// File metadata (permissions, etc.)
#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
}

impl Default for FileMetadata {
    fn default() -> Self {
        FileMetadata {
            mode: 0o644,
            uid: 0,
            gid: 0,
        }
    }
}

impl Initramfs {
    /// Create a new empty initramfs
    pub fn new() -> Self {
        Initramfs {
            files: HashMap::new(),
            metadata: HashMap::new(),
        }
    }

    /// Get the content of a file
    pub fn get_file(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(|v| v.as_slice())
    }

    /// Add or update a file
    pub fn add_file(&mut self, path: String, content: Vec<u8>, metadata: FileMetadata) {
        self.files.insert(path.clone(), content);
        self.metadata.insert(path, metadata);
    }

    /// Remove a file
    pub fn remove_file(&mut self, path: &str) -> Option<Vec<u8>> {
        self.metadata.remove(path);
        self.files.remove(path)
    }

    /// Save the initramfs to a directory
    pub fn save(&self, dir: &Path) -> Result<()> {
        fs::create_dir_all(dir)?;
        
        for (path, content) in &self.files {
            let file_path = dir.join(path.trim_start_matches('/'));
            
            // Check if this is a directory entry (mode & 0o040000 == directory)
            let is_directory = if let Some(metadata) = self.metadata.get(path) {
                (metadata.mode & 0o040000) != 0
            } else {
                false
            };
            
            if is_directory {
                // Create directory
                fs::create_dir_all(&file_path)?;
            } else {
                // Create parent directories
                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                
                // Create file
                let mut file = File::create(&file_path)?;
                file.write_all(content)?;
                
                // Set permissions if available
                #[cfg(unix)]
                if let Some(metadata) = self.metadata.get(path) {
                    use std::os::unix::fs::PermissionsExt;
                    let perms = std::fs::Permissions::from_mode(metadata.mode);
                    fs::set_permissions(&file_path, perms)?;
                }
            }
        }
        
        Ok(())
    }
}

impl Default for Initramfs {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating modified initramfs
pub struct InitramfsBuilder {
    initramfs: Initramfs,
}

impl InitramfsBuilder {
    /// Create a new builder from an existing initramfs
    pub fn from_initramfs(initramfs: Initramfs) -> Result<Self> {
        Ok(InitramfsBuilder { initramfs })
    }

    /// Create a new empty builder
    pub fn new() -> Self {
        InitramfsBuilder {
            initramfs: Initramfs::new(),
        }
    }

    /// Add a file with default permissions
    pub fn add_file(&mut self, path: &str, content: &[u8]) -> Result<&mut Self> {
        self.add_file_with_mode(path, content, 0o644)
    }

    /// Add an executable file
    pub fn add_executable(&mut self, path: &str, content: &[u8]) -> Result<&mut Self> {
        self.add_file_with_mode(path, content, 0o755)
    }

    /// Add a file with specific permissions
    pub fn add_file_with_mode(&mut self, path: &str, content: &[u8], mode: u32) -> Result<&mut Self> {
        let metadata = FileMetadata {
            mode,
            uid: 0,
            gid: 0,
        };
        self.initramfs.add_file(path.to_string(), content.to_vec(), metadata);
        Ok(self)
    }

    /// Remove a file from the initramfs
    pub fn remove_file(&mut self, path: &str) -> Result<&mut Self> {
        self.initramfs.remove_file(path);
        Ok(self)
    }

    /// Replace the init script with a custom one
    pub fn replace_init(&mut self, init_script: &[u8]) -> Result<&mut Self> {
        self.remove_file("init")?;
        self.add_executable("init", init_script)
    }

    /// Add a custom payload script to bin/
    pub fn add_payload_script(&mut self, name: &str, script: &[u8]) -> Result<&mut Self> {
        let path = format!("bin/{}", name);
        self.add_executable(&path, script)
    }

    /// Build the final initramfs
    pub fn build(self) -> Result<Initramfs> {
        Ok(self.initramfs)
    }
}

impl Default for InitramfsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract initramfs from a kernel image
fn extract_initramfs_from_kernel(kernel_data: &[u8]) -> Result<Initramfs> {
    // Find gzip compressed data
    let gzip_offset = find_gzip_magic(kernel_data)
        .ok_or_else(|| ShimError::GzipNotFound(kernel_data.len()))?;
    
    // Decompress the kernel
    let mut decoder = GzDecoder::new(&kernel_data[gzip_offset..]);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)
        .map_err(|e| ShimError::InvalidKernel(format!("Failed to decompress gzip data: {}", e)))?;
    
    // Find all CPIO archives in decompressed data
    let cpio_positions = find_all_cpio_magic(&decompressed);
    
    if cpio_positions.is_empty() {
        return Err(ShimError::CpioNotFound(decompressed.len()));
    }
    
    // Try each CPIO archive until we find one with files
    let mut last_error = None;
    for cpio_offset in cpio_positions {
        match parse_cpio(&decompressed[cpio_offset..]) {
            Ok(initramfs) => {
                // Successfully parsed and has files
                return Ok(initramfs);
            }
            Err(e) => {
                // Store the error and try next CPIO archive
                last_error = Some(e);
                continue;
            }
        }
    }
    
    // None of the CPIO archives worked
    Err(last_error.unwrap_or_else(|| 
        ShimError::InvalidInitramfs("All CPIO archives failed to parse".to_string())
    ))
}

/// Find gzip magic bytes (0x1f 0x8b) in data
fn find_gzip_magic(data: &[u8]) -> Option<usize> {
    data.windows(2)
        .position(|window| window == [0x1f, 0x8b])
}

/// Find all CPIO magic bytes positions in data
/// CPIO ASCII format starts with "070701" or "070702"
fn find_all_cpio_magic(data: &[u8]) -> Vec<usize> {
    let magic1 = b"070701";
    let magic2 = b"070702";
    
    let mut positions = Vec::new();
    for i in 0..data.len().saturating_sub(6) {
        let window = &data[i..i+6];
        if window == magic1 || window == magic2 {
            positions.push(i);
        }
    }
    positions
}

/// Find first CPIO magic bytes in data (for backwards compatibility)
fn find_cpio_magic(data: &[u8]) -> Option<usize> {
    let magic1 = b"070701";
    let magic2 = b"070702";
    
    data.windows(6)
        .position(|window| window == magic1 || window == magic2)
}

/// Parse a CPIO archive into an Initramfs
fn parse_cpio(data: &[u8]) -> Result<Initramfs> {
    let mut initramfs = Initramfs::new();
    let mut reader = Cursor::new(data);
    
    loop {
        // Read CPIO header (newc format)
        let mut header = vec![0u8; 110];
        if reader.read_exact(&mut header).is_err() {
            break;
        }
        
        let header_str = String::from_utf8_lossy(&header);
        
        // Check magic
        if !header_str.starts_with("070701") && !header_str.starts_with("070702") {
            break;
        }
        
        // Parse header fields (hex ASCII) - if this fails, break the loop
        let mode = match parse_hex(&header_str[14..22]) {
            Ok(m) => m as u32,
            Err(_) => break,
        };
        let filesize = match parse_hex(&header_str[54..62]) {
            Ok(s) => s,
            Err(_) => break,
        };
        let namesize = match parse_hex(&header_str[94..102]) {
            Ok(s) => s,
            Err(_) => break,
        };
        
        // Validate namesize to prevent issues
        if namesize == 0 {
            break;
        }
        
        // Read filename
        let mut name_bytes = vec![0u8; namesize];
        if reader.read_exact(&mut name_bytes).is_err() {
            break;
        }
        
        // Safely handle the name parsing
        if namesize == 0 {
            break;
        }
        let name = String::from_utf8_lossy(&name_bytes[..namesize-1]).to_string();
        
        // Skip padding to align to 4 bytes (only if needed)
        let name_padding = (4 - (namesize % 4)) % 4;
        if name_padding > 0 {
            let mut padding_buf = vec![0u8; name_padding];
            // If padding read fails, just break
            if reader.read_exact(&mut padding_buf).is_err() {
                break;
            }
        }
        
        // Check for trailer (end of archive)
        if name == "TRAILER!!!" {
            break;
        }
        
        // Read file content
        let mut content = vec![0u8; filesize];
        if reader.read_exact(&mut content).is_err() {
            break;
        }
        
        // Skip padding to align to 4 bytes (only if needed)
        let content_padding = (4 - (filesize % 4)) % 4;
        if content_padding > 0 {
            let mut padding_buf = vec![0u8; content_padding];
            // If padding read fails, just break
            if reader.read_exact(&mut padding_buf).is_err() {
                break;
            }
        }
        
        // Add to initramfs
        let metadata = FileMetadata {
            mode,
            uid: 0,
            gid: 0,
        };
        initramfs.add_file(name, content, metadata);
    }
    
    // Ensure we extracted at least some files
    if initramfs.files.is_empty() {
        return Err(ShimError::InvalidInitramfs(
            "No files found in CPIO archive - archive may be empty or corrupted".to_string()
        ));
    }
    
    Ok(initramfs)
}

/// Parse hex string to usize
fn parse_hex(s: &str) -> Result<usize> {
    usize::from_str_radix(s, 16)
        .map_err(|e| ShimError::InvalidInitramfs(format!("Failed to parse hex '{}': {}", s, e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initramfs_builder() {
        let mut builder = InitramfsBuilder::new();
        builder.add_file("test.txt", b"hello").unwrap();
        builder.add_executable("bin/script.sh", b"#!/bin/sh\necho test\n").unwrap();
        
        let initramfs = builder.build().unwrap();
        assert_eq!(initramfs.get_file("test.txt"), Some(&b"hello"[..]));
        assert_eq!(initramfs.files.len(), 2);
    }

    #[test]
    fn test_find_gzip_magic() {
        let data = vec![0x00, 0x00, 0x1f, 0x8b, 0x08, 0x00];
        assert_eq!(find_gzip_magic(&data), Some(2));
    }

    #[test]
    fn test_find_cpio_magic() {
        let data = b"some data 070701 rest";
        assert_eq!(find_cpio_magic(data), Some(10));
    }
}
