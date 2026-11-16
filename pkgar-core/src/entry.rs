//! The packed structs represent the on-disk format of pkgar
use core::fmt::Display;

use alloc::boxed::Box;
use blake3::Hash;
use bytemuck::{Pod, Zeroable};

use crate::{Error, Mode};

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(packed, C)]
pub struct Entry {
    /// Blake3 sum of the file data
    pub blake3: [u8; 32],
    /// Offset of file data in the data portion
    pub offset: u64,
    /// Size in bytes of the file data in the data portion
    pub size: u64,
    /// Unix permissions (user, group, other with read, write, execute)
    pub mode: u32,
    /// NUL-terminated relative path from extract directory
    pub path: [u8; 256],
}

impl Display for Entry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let (offset, size, mode) = (self.offset, self.size, self.mode);
        write!(
            f,
            "path={:?} hash={} offset={} size={} mode={:o}",
            alloc::string::String::from_utf8(self.path_bytes().into()).unwrap_or_default(),
            self.blake3().to_hex(),
            offset,
            size,
            mode
        )
    }
}

impl Entry {
    pub fn new_uninit(path: &[u8], mode: Mode) -> Result<Self, Error> {
        let mut path_bytes = [0; 256];
        let len = core::cmp::min(path_bytes.len() - 1, path.len());
        path_bytes[..len].copy_from_slice(&path[..len]);
        if len != path.len() {
            return Err(Error::OverflowPath(Box::new(path_bytes)));
        }

        Ok(Self {
            blake3: [0; 32],
            offset: 0,
            size: 0,
            mode: mode.bits(),
            path: path_bytes,
        })
    }

    pub fn init(&mut self, blake3: [u8; 32], offset: u64, size: u64) {
        self.blake3 = blake3;
        self.offset = offset;
        self.size = size;
    }

    pub fn blake3(&self) -> Hash {
        Hash::from(self.blake3)
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn mode(&self) -> Result<Mode, Error> {
        Mode::from_bits(self.mode).ok_or(Error::InvalidMode(self.mode))
    }

    /// Retrieve the path, ending at the first NUL
    pub fn path_bytes(&self) -> &[u8] {
        let mut i = 0;
        while i < self.path.len() {
            if self.path[i] == 0 {
                break;
            }
            i += 1;
        }
        &self.path[..i]
    }
}
