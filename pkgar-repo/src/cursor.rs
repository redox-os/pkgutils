use std::{
    collections::BTreeMap,
    io::{Error as IoError, ErrorKind, Read, Result as IoResult, Seek, SeekFrom},
};

/// Implements memory caching based on known length.
///
/// Unlike [`std::io::BufReader`] which is optimized for reading, this also optimized
/// for seek and guarantees that cached memory will never be flushed on any operation.
pub struct BufCacheReader<R> {
    inner: R,
    chunk_size: u64,
    len: u64,
    cache: BTreeMap<u64, Vec<u8>>,
    cursor: u64,
}

impl<R: Read + Seek> BufCacheReader<R> {
    pub fn new(inner: R, len: u64, chunk: u64) -> Self {
        Self {
            inner,
            chunk_size: chunk,
            len,
            cache: BTreeMap::new(),
            cursor: 0,
        }
    }

    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl<R: Read + Seek> Read for BufCacheReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        if buf.is_empty() || self.cursor >= self.len {
            return Ok(0);
        }

        let chunk_idx = self.cursor / self.chunk_size;
        let chunk_offset = (self.cursor % self.chunk_size) as usize;

        if !self.cache.contains_key(&chunk_idx) {
            let chunk_start = chunk_idx * self.chunk_size;
            self.inner.seek(SeekFrom::Start(chunk_start))?;

            let remaining = self.len.saturating_sub(chunk_start);
            let to_read = std::cmp::min(self.chunk_size, remaining) as usize;

            let mut new_chunk = vec![0u8; to_read];

            self.inner.read_exact(&mut new_chunk)?;

            self.cache.insert(chunk_idx, new_chunk);
        }

        let chunk = self.cache.get(&chunk_idx).unwrap();
        let available = chunk.len() - chunk_offset;

        let to_copy = std::cmp::min(buf.len(), available);

        buf[..to_copy].copy_from_slice(&chunk[chunk_offset..chunk_offset + to_copy]);

        self.cursor += to_copy as u64;

        Ok(to_copy)
    }
}

impl<R: Read + Seek> Seek for BufCacheReader<R> {
    fn seek(&mut self, pos: SeekFrom) -> IoResult<u64> {
        match pos {
            SeekFrom::Start(offset) => {
                self.cursor = offset;
            }
            SeekFrom::Current(delta) => {
                let new_cursor = if delta < 0 {
                    self.cursor.checked_sub(delta.unsigned_abs())
                } else {
                    self.cursor.checked_add(delta as u64)
                };
                self.cursor = new_cursor
                    .ok_or_else(|| IoError::new(ErrorKind::InvalidInput, "Invalid seek"))?;
            }
            SeekFrom::End(_) => {
                return Err(IoError::new(ErrorKind::Unsupported, "Unimplemented"));
            }
        }
        Ok(self.cursor)
    }
}
