//! Storage wrappers that use [bounded](super::bounded) implementations when the content length is
//! not known. In this scenario, it is assumed that the source is an infinite
//! stream.
//!
//! This module is useful when you want to need to support both infinite and finite streams while
//! avoiding the overhead of using a bounded buffer when it's not necessary.

use std::io::{self, Read, Seek, SeekFrom, Write};
use std::num::NonZeroUsize;

use super::bounded::{BoundedStorageProvider, BoundedStorageReader, BoundedStorageWriter};
use super::{StorageProvider, StorageReader, StorageWriter};

/// Creates an [`AdaptiveStorageReader`] based in the supplied content length
#[derive(Clone, Debug)]
pub struct AdaptiveStorageProvider<T>
where
    T: StorageProvider,
{
    size: NonZeroUsize,
    inner: T,
}

impl<T> AdaptiveStorageProvider<T>
where
    T: StorageProvider,
{
    /// Creates a new [`AdaptiveStorageProvider`]. The supplied size is used to construct a
    /// [`BoundedStorageReader`] when the stream doesn't have a known content length.
    pub fn new(inner: T, size: NonZeroUsize) -> Self {
        Self { size, inner }
    }
}

/// Reader created by an [`AdaptiveStorageProvider`].
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

    fn into_reader_writer(
        self,
        content_length: Option<u64>,
    ) -> io::Result<(Self::Reader, Self::Writer)> {
        if content_length.is_some() {
            let (reader, writer) = self.inner.into_reader_writer(content_length)?;
            let reader = AdaptiveStorageReader::Unbounded(reader);
            let writer = AdaptiveStorageWriter::Unbounded(writer);
            Ok((reader, writer))
        } else {
            let provider = BoundedStorageProvider::new(self.inner, self.size);
            let (reader, writer) = provider.into_reader_writer(content_length)?;
            let reader = AdaptiveStorageReader::Bounded(reader);
            let writer = AdaptiveStorageWriter::Bounded(writer);
            Ok((reader, writer))
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
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        match self {
            Self::Bounded(inner) => inner.seek(position),
            Self::Unbounded(inner) => inner.seek(position),
        }
    }
}

/// Write handle created by an [`AdaptiveStorageReader`].
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
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        match self {
            Self::Bounded(inner) => inner.seek(position),
            Self::Unbounded(inner) => inner.seek(position),
        }
    }
}
