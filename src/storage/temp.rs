//! Storage implementations for reading and writing to a temporary file. If the content length is
//! known, the buffer size will be initialized to the content length, but the buffer will expand
//! beyond that if required.
use std::fs::File;
use std::io::{self, BufReader, Read, Seek};
use std::path::PathBuf;

use tempfile::NamedTempFile;

use super::{StorageProvider, StorageReader};
use crate::WrapIoResult;

/// Creates a [TempStorageReader] backed by a temporary file
#[derive(Default, Clone, Debug)]
pub struct TempStorageProvider {
    storage_dir: Option<PathBuf>,
}

impl TempStorageProvider {
    /// Creates a new [TempStorageProvider] that creates temporary files in the OS-specific default
    /// location.
    pub fn new() -> Self {
        Self { storage_dir: None }
    }

    /// Creates a new [TempStorageProvider] that creates temporary files in the specified location.
    pub fn new_in(path: impl Into<PathBuf>) -> Self {
        Self {
            storage_dir: Some(path.into()),
        }
    }
}

impl StorageProvider for TempStorageProvider {
    type Reader = TempStorageReader;

    fn create_reader(&self, _content_length: Option<u64>) -> io::Result<Self::Reader> {
        let tempfile = if let Some(dir) = &self.storage_dir {
            NamedTempFile::new_in(dir)
        } else {
            NamedTempFile::new()
        }
        .wrap_err("error creating temp file")?;

        let handle = tempfile.reopen().wrap_err("error reopening temp file")?;
        Ok(TempStorageReader {
            reader: BufReader::new(tempfile),
            handle,
        })
    }
}

/// Reader created by a [TempStorageProvider]. Reads from a temporary file.
#[derive(Debug)]
pub struct TempStorageReader {
    reader: BufReader<NamedTempFile>,
    handle: File,
}

impl Read for TempStorageReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reader.read(buf)
    }
}

impl Seek for TempStorageReader {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        self.reader.seek(pos)
    }
}

impl StorageReader for TempStorageReader {
    type Writer = File;

    fn writer(&self) -> io::Result<Self::Writer> {
        self.handle
            .try_clone()
            .wrap_err("error cloning temporary file")
    }
}
