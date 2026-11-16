use alloc::vec;
use alloc::vec::Vec;
use bytemuck::Zeroable;
use core::convert::TryFrom;

use dryoc::classic::crypto_sign_ed25519::PublicKey;

use crate::{Entry, Error, HEADER_SIZE, Header};

pub trait PackageSrc {
    type Err: From<Error>;

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, Self::Err>;

    fn header(&self) -> Header;

    /// Users of implementors of `PackageSrc` should use `header` instead of `read_header` for
    /// cheap header access.
    /// Implementors of `PackageSrc` should call this function during initialization and store
    /// the result to pass out with `header`.
    fn read_header(&mut self, public_key: &PublicKey) -> Result<Header, Self::Err> {
        let mut header_data = [0; HEADER_SIZE];
        self.read_at(0, &mut header_data)?;
        let header = Header::new(&header_data, public_key)?;
        Ok(*header)
    }

    fn read_entries(&mut self) -> Result<Vec<Entry>, Self::Err> {
        let header = self.header();
        let entries_size = header.entries_size()?;
        let mut entries_data = vec![0; entries_size];
        self.read_at(HEADER_SIZE as u64, &mut entries_data)?;
        let entries = header.entries(&entries_data)?;
        Ok(entries.to_vec())
    }

    /// Read from this src at a given entry's data with a given offset within that entry
    fn read_entry(
        &mut self,
        entry: &Entry,
        offset: u64,
        buf: &mut [u8],
    ) -> Result<usize, Self::Err> {
        let end = Self::calculate_end(&entry, offset, buf)?;

        if end == 0 {
            return Ok(0);
        }

        let offset = self.header().total_size()? as u64 + entry.offset + offset;

        self.read_at(offset, &mut buf[..end])
    }

    /// Helper to get end of buffer relative to entry
    fn calculate_end(entry: &Entry, offset: u64, buf: &[u8]) -> Result<usize, Self::Err> {
        if offset > entry.size {
            return Ok(0);
        }
        let mut end = usize::try_from(entry.size - offset).map_err(Error::TryFromInt)?;
        if end > buf.len() {
            end = buf.len();
        }
        Ok(end)
    }

    /// Helper to get range relative to buffer
    fn calculate_range(
        src_len: usize,
        offset: u64,
        buf: &[u8],
    ) -> Result<(usize, usize), Self::Err> {
        let start = usize::try_from(offset).map_err(Error::TryFromInt)?;
        if start >= src_len {
            return Ok((0, 0));
        }
        let mut end = start.checked_add(buf.len()).ok_or(Error::Overflow)?;
        if end > src_len {
            end = src_len;
        }

        Ok((start, end))
    }
}

//TODO: Test this impl...
pub struct PackageBuf<'a> {
    src: &'a [u8],
    header: Header,
}

impl<'a> PackageBuf<'a> {
    pub fn new(src: &'a [u8], public_key: &PublicKey) -> Result<PackageBuf<'a>, Error> {
        let mut new = PackageBuf {
            src,
            header: Header::zeroed(),
        };
        new.header = *Header::new(new.src, public_key)?;
        Ok(new)
    }
}

impl PackageSrc for PackageBuf<'_> {
    type Err = Error;

    fn header(&self) -> Header {
        self.header
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, Error> {
        let (start, end) = Self::calculate_range(self.src.len(), offset, buf)?;
        buf.copy_from_slice(&self.src[start..end]);
        Ok(buf.len())
    }
}
