//! Storage wrappers that use [bounded](super::bounded) implementations when the content length is
//! not known. In this scenario, it is assumed that the source is an infinite
//! stream.
//!
//! This module is useful when you want to need to support both infinite and finite streams without
//! explicitly checking.

use std::io::{self, Read, Seek, SeekFrom, Write};
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use super::bounded::{BoundedStorageProvider, BoundedStorageReader, BoundedStorageWriter};
use super::{StorageProvider, StorageReader, StorageWriter};

/// Creates an [AdaptiveStorageReader] based in the supplied content length
#[derive(Clone, Debug)]
pub struct AdaptiveStorageProvider<T>
where
    T: StorageProvider,
{
    size: NonZeroUsize,
    inner: T,
    // refactoring reader/writer
    // TODO remove after refactor merges reader/writer creation
    bounded: Arc<Mutex<Option<BoundedStorageProvider<T>>>>,
}

impl<T> AdaptiveStorageProvider<T>
where
    T: StorageProvider,
{
    /// Creates a new [AdaptiveStorageProvider]. The supplied size is used to construct a
    /// [BoundedStorageReader] when the stream doesn't have a known content length.
    pub fn new(inner: T, size: NonZeroUsize) -> Self {
        Self {
            inner,
            size,
            bounded: Arc::new(Mutex::new(None)),
        }
    }
}

/// Reader created by an [AdaptiveStorageProvider].
#[derive(Debug)]
pub enum AdaptiveStorageReader<T: StorageReader> {
    /// Bounded reader used for infinite streams.
    Bounded(BoundedStorageReader<T>),
    /// Unbounded reader used for finite streams.
    Unbounded(T),
}

impl<T> StorageProvider for AdaptiveStorageProvider<T>
where
    T: StorageProvider,
{
    type Reader = AdaptiveStorageReader<T::Reader>;
    type Writer = AdaptiveStorageWriter<T::Writer>;

    fn create_reader(&self, content_length: Option<u64>) -> io::Result<Self::Reader> {
        if let Some(content_length) = content_length {
            *self.bounded.lock().unwrap() = None;
            Ok(AdaptiveStorageReader::Unbounded(
                self.inner.create_reader(Some(content_length))?,
            ))
        } else {
            let provier = BoundedStorageProvider::new(self.inner.clone(), self.size);
            let reader = AdaptiveStorageReader::Bounded(provier.create_reader(None)?);
            *self.bounded.lock().unwrap() = Some(provier);
            Ok(reader)
        }
    }
    fn writer(&self) -> io::Result<Self::Writer> {
        match self.bounded.lock().unwrap().as_ref() {
            Some(provier) => {
                Ok(AdaptiveStorageWriter::Bounded(provier.writer()?))
            }
            None => Ok(AdaptiveStorageWriter::Unbounded(self.inner.writer()?)),
        }
    }
}

impl<T> Read for AdaptiveStorageReader<T>
where
    T: StorageReader,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Bounded(inner) => inner.read(buf),
            Self::Unbounded(inner) => inner.read(buf),
        }
    }
}

impl<T> Seek for AdaptiveStorageReader<T>
where
    T: StorageReader,
{
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match self {
            Self::Bounded(inner) => inner.seek(pos),
            Self::Unbounded(inner) => inner.seek(pos),
        }
    }
}

impl<T> StorageReader for AdaptiveStorageReader<T> where T: StorageReader {}

/// Write handle created by an [AdaptiveStorageReader].
#[derive(Debug)]
pub enum AdaptiveStorageWriter<T: StorageWriter> {
    /// Bounded reader used for infinite streams.
    Bounded(BoundedStorageWriter<T>),
    /// Unbounded reader used for finite streams.
    Unbounded(T),
}

impl<T> Write for AdaptiveStorageWriter<T>
where
    T: StorageWriter,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Bounded(inner) => inner.write(buf),
            Self::Unbounded(inner) => inner.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Bounded(inner) => inner.flush(),
            Self::Unbounded(inner) => inner.flush(),
        }
    }
}

impl<T> Seek for AdaptiveStorageWriter<T>
where
    T: StorageWriter,
{
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match self {
            Self::Bounded(inner) => inner.seek(pos),
            Self::Unbounded(inner) => inner.seek(pos),
        }
    }
}
