#![no_std]
extern crate alloc;

use core::mem;

use bitflags::bitflags;

pub use dryoc::{
    self,
    classic::crypto_sign_ed25519::{PublicKey, SecretKey, Signature},
};

pub use bytemuck::Zeroable;

pub use crate::entry::Entry;
pub use crate::error::Error;
pub use crate::flags::{Architecture, DataVersion, HeaderFlags, Packaging};
pub use crate::header::Header;
pub use crate::package::{PackageBuf, PackageSrc};

mod entry;
mod error;
mod flags;
mod header;
mod package;

pub const HEADER_SIZE: usize = mem::size_of::<Header>();
pub const ENTRY_SIZE: usize = mem::size_of::<Entry>();

bitflags! {
    /// Ensures that all platforms use the same mode defines.
     #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Mode: u32 {
        const PERM = 0o007777;
        const KIND = 0o170000;
        const FILE = 0o100000;
        const SYMLINK = 0o120000;
    }
}

impl Mode {
    /// Get a new Mode. PERM and KIND must be not zero.
    pub fn new(perm: u32, kind: u32) -> Result<Self, Error> {
        let perm = Mode::from_bits_retain(perm & Self::PERM.bits());
        let kind = Mode::from_bits_retain(kind & Self::KIND.bits());
        if kind.is_empty() || perm.is_empty() {
            return Err(Error::InvalidMode(kind.bits() | perm.bits()));
        }
        Ok(kind | perm)
    }

    /// Get a new Mode. Guaranteed to have FILE bit set. PERM must not be zero.
    pub fn new_file(perm: u32, is_file: bool, is_symlink: bool) -> Result<Self, Error> {
        let kind = if is_symlink {
            Self::SYMLINK.bits()
        } else if is_file {
            Self::FILE.bits()
        } else {
            0
        };
        Self::new(perm, kind)
    }

    /// Only any kind bits
    pub fn kind(self) -> Mode {
        self & Mode::KIND
    }

    /// Only any permissions bits
    pub fn perm(self) -> Mode {
        self & Mode::PERM
    }
}

#[cfg(test)]
mod tests {
    use core::mem;

    use crate::{ENTRY_SIZE, Entry, HEADER_SIZE, Header};

    #[test]
    fn header_size() {
        assert_eq!(mem::size_of::<Header>(), 136);
        assert_eq!(HEADER_SIZE, 136);
    }

    #[test]
    fn entry_size() {
        assert_eq!(mem::size_of::<Entry>(), 308);
        assert_eq!(ENTRY_SIZE, 308);
    }
}
