// Shimboot - Boot desktop Linux from a Chrome OS RMA shim
// Copyright (C) 2025 ading2210
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

pub mod common;
pub mod image_utils;
pub mod shim_utils;
pub mod build;
pub mod build_complete;
pub mod build_rootfs;
pub mod patch_rootfs;
pub mod build_squashfs;
pub mod commands;
