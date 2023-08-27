//! Storage implementations for reading and writing to an in-memory buffer. If the content length is
//! known, the buffer size will be initialized to the content length, but the buffer will expand
//! beyond that if required.
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;

use super::{StorageProvider, StorageReader};

/// Creates a [MemoryStorage] with an initial size based on the supplied content length.
#[derive(Default, Clone, Debug)]
pub struct MemoryStorageProvider {}

impl StorageProvider for MemoryStorageProvider {
    type Reader = MemoryStorage;
    fn create_reader(&self, content_length: Option<u64>) -> io::Result<Self::Reader> {
        Ok(MemoryStorage {
            inner: Arc::new(RwLock::new(vec![0; content_length.unwrap_or(0) as usize])),
            pos: 0,
            written: Arc::new(AtomicUsize::new(0)),
        })
    }
}

/// Simple threadsafe in-memory buffer that supports the standard IO traits.
#[derive(Debug)]
pub struct MemoryStorage {
    inner: Arc<RwLock<Vec<u8>>>,
    pos: usize,
    written: Arc<AtomicUsize>,
}

impl StorageReader for MemoryStorage {
    type Writer = Self;
    fn writer(&self) -> io::Result<Self::Writer> {
        Ok(Self {
            inner: self.inner.clone(),
            pos: 0,
            written: self.written.clone(),
        })
    }
}

impl Read for MemoryStorage {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let inner = self.inner.read();

        let available_len = (inner.len() - self.pos).min(self.written.load(Ordering::SeqCst));
        let read_len = available_len.min(buf.len());
        buf[..read_len].copy_from_slice(&inner[self.pos..self.pos + read_len]);
        self.pos += read_len;
        Ok(read_len)
    }
}

impl Seek for MemoryStorage {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let len = self.inner.read().len();
        let new_pos = match pos {
            SeekFrom::Start(pos) => pos as usize,
            SeekFrom::Current(from_current) => ((self.pos as i64) + from_current) as usize,
            SeekFrom::End(from_end) => (len as i64 + from_end) as usize,
        };

        self.pos = new_pos;
        Ok(new_pos as u64)
    }
}

impl Write for MemoryStorage {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut inner = self.inner.write();

        if self.pos + buf.len() > inner.len() {
            inner.resize(self.pos + buf.len(), 0);
        }

        inner[self.pos..self.pos + buf.len()].copy_from_slice(buf);

        self.pos += buf.len();
        self.written.fetch_add(buf.len(), Ordering::SeqCst);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
