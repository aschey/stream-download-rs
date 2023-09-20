//! Storage implementations for reading and writing to a user specified permanent file. Seeking is
//! still supported, when all data in front of the current seekpoint has been downloaded any
//! missing parts in front are downloaded. To keep track of the downloaded data every
//! [`PermanentStorageProvider`] creates a .progress file.
use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Seek, Write};
use std::path::PathBuf;
use std::{fs, io};

use crate::StorageProvider;

mod progress;
use progress::Progress;
use rangemap::RangeSet;

use super::StorageWriter;

/// Used to create a [`PermanentStorageReader`] and [`PermanentStorageWriter`]
#[derive(Default, Clone, Debug)]
pub struct PermanentStorageProvider {
    path: PathBuf,
    force_restart: bool,
}

///
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Not a valid path, path may not end in .progress
    #[error("Not a valid path, path may not end in .progress")]
    InvalidPath,
}

impl PermanentStorageProvider {
    /// Creates a new [`PermanentStorageProvider`] to download to a user specified path. It removes any existing file throwing away any
    /// download progress made.
    pub fn restart(path: PathBuf) -> Result<Self, Error> {
        Self::new(path, true)
    }
    /// Creates a new [`PermanentStorageProvider`] to download to a user specified path. It tries to continue if any file existed. If it can not it starts downloading from scratch overwriting existing data.
    pub fn start(path: PathBuf) -> Result<Self, Error> {
        Self::new(path, false)
    }

    fn new(path: PathBuf, force_restart: bool) -> Result<Self, Error> {
        if path.extension() == Some(OsStr::new("progress")) {
            return Err(Error::InvalidPath);
        }
        Ok(Self {
            path,
            force_restart,
        })
    }

    fn setup_files(
        &self,
        content_length: Option<u64>,
    ) -> io::Result<(BufReader<File>, BufWriter<File>, Progress)> {
        let file = fs::OpenOptions::new().write(true).open(&self.path)?;
        if let Some(content_length) = content_length {
            /* TODO: handle error (for example disk full) <18-09-23> */
            file.set_len(content_length).unwrap()
        }

        let writer = BufWriter::new(file);

        let file = fs::OpenOptions::new()
            .read(true)
            .truncate(false)
            .open(&self.path)?;
        let reader = BufReader::new(file);

        let progress = if self.force_restart {
            Progress::restart(&self.path)
        } else {
            Progress::start_or_continue(&self.path)
        }?;

        Ok((reader, writer, progress))
    }
}

impl StorageProvider for PermanentStorageProvider {
    type Reader = PermanentStorageReader;
    type Writer = PermanentStorageWriter;

    fn into_reader_writer(
        self,
        content_length: Option<u64>,
    ) -> std::io::Result<(Self::Reader, Self::Writer)> {
        let (reader, writer, progress) = self.setup_files(content_length)?;
        let initially_downloaded = progress.downloaded()?;

        Ok((
            PermanentStorageReader { reader },
            PermanentStorageWriter {
                writer,
                progress,
                initially_downloaded,
            },
        ))
    }
}

/// Reader created by a [`PermanentStorageProvider`]. Reads from a file on disk.
pub struct PermanentStorageReader {
    reader: BufReader<fs::File>,
}

impl Read for PermanentStorageReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.reader.read(buf)
    }
}

impl Seek for PermanentStorageReader {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.reader.seek(pos)
    }
}

/// Writer created by a [`PermanentStorageProvider`]. Writes to a file on disk.
pub struct PermanentStorageWriter {
    writer: BufWriter<fs::File>,
    progress: Progress,
    initially_downloaded: RangeSet<u64>,
}

impl StorageWriter for PermanentStorageWriter {
    fn downloaded(&self) -> rangemap::RangeSet<u64> {
        self.initially_downloaded.clone()
    }
}

impl Write for PermanentStorageWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

impl Seek for PermanentStorageWriter {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        let curr = self.writer.stream_position()?;
        self.progress.finish_section(curr)?;
        self.writer.seek(pos)
    }
}
