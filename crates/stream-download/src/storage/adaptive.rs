//! Storage wrappers that adaptively choose between two storage providers based on content length.
//!
//! This module provides a way to use different storage strategies depending on the stream's
//! characteristics:
//! - For infinite streams: uses the bounded storage provider
//! - For finite streams smaller than the buffer size: uses the bounded storage provider
//! - For finite streams larger than the buffer size: uses the unbounded storage provider
//!
//! This is particularly useful for optimizing resource usage - for example, using memory-backed
//! storage for small streams while falling back to file-backed storage for larger ones to avoid
//! excessive memory usage.

use std::io::{self, Read, Seek, SeekFrom, Write};
use std::num::NonZeroUsize;

use super::bounded::BoundedStorageProvider;
use super::{StorageProvider, StorageReader, StorageWriter};

/// Provides adaptive storage selection based on stream characteristics.
///
/// Takes two storage providers:
/// - `B`: Used for bounded storage for infinite streams or small finite streams
/// - `U`: Used for unbounded storage for large finite streams
#[derive(Clone, Debug)]
pub struct AdaptiveStorageProvider<B, U>
where
    B: StorageProvider,
    U: StorageProvider,
{
    buffer_size: NonZeroUsize,
    /// Storage provider used for infinite streams or finite streams smaller than `buffer_size`
    bounded: B,
    /// Storage provider used for finite streams larger than `buffer_size`
    unbounded: U,
}

impl<B, U> AdaptiveStorageProvider<B, U>
where
    B: StorageProvider,
    U: StorageProvider,
{
    /// Creates a new [`AdaptiveStorageProvider`]. The supplied buffer size is used to construct a
    /// [`BoundedStorageReader`] when the stream's content length is unknown or smaller than the
    /// buffer size.
    pub fn new(bounded: B, unbounded: U, buffer_size: NonZeroUsize) -> Self {
        Self {
            buffer_size,
            bounded,
            unbounded,
        }
    }
}

impl<P> AdaptiveStorageProvider<P, P>
where
    P: StorageProvider,
{
    /// Creates a new [`AdaptiveStorageProvider`] using the same provider type for both bounded
    /// and unbounded storage.
    pub fn with_same_provider(provider: P, buffer_size: NonZeroUsize) -> Self
    where
        P: Clone,
    {
        Self::new(provider.clone(), provider, buffer_size)
    }
}

impl<B, U> StorageProvider for AdaptiveStorageProvider<B, U>
where
    B: StorageProvider,
    U: StorageProvider,
    B::Reader: 'static,
    U::Reader: 'static,
{
    type Reader = AdaptiveReader;
    type Writer = AdaptiveWriter;

    fn into_reader_writer(
        self,
        content_length: Option<u64>,
    ) -> io::Result<(Self::Reader, Self::Writer)> {
        match content_length {
            None => {
                // For infinite streams, use bounded storage
                let provider = BoundedStorageProvider::new(self.bounded, self.buffer_size);
                let (reader, writer) = provider.into_reader_writer(None)?;
                Ok((
                    AdaptiveReader {
                        inner: AdaptiveReaderType::Bounded(Box::new(reader)),
                    },
                    AdaptiveWriter {
                        inner: AdaptiveWriterType::Bounded(Box::new(writer)),
                    },
                ))
            }
            Some(length) => {
                if length < self.buffer_size.get() as u64 {
                    // Small enough for bounded storage
                    let (reader, writer) = self.bounded.into_reader_writer(Some(length))?;
                    Ok((
                        AdaptiveReader {
                            inner: AdaptiveReaderType::Bounded(Box::new(reader)),
                        },
                        AdaptiveWriter {
                            inner: AdaptiveWriterType::Bounded(Box::new(writer)),
                        },
                    ))
                } else {
                    // Too large, use unbounded storage
                    let (reader, writer) = self.unbounded.into_reader_writer(Some(length))?;
                    Ok((
                        AdaptiveReader {
                            inner: AdaptiveReaderType::Unbounded(Box::new(reader)),
                        },
                        AdaptiveWriter {
                            inner: AdaptiveWriterType::Unbounded(Box::new(writer)),
                        },
                    ))
                }
            }
        }
    }
}

/// Reader that adaptively uses either bounded or unbounded storage
pub struct AdaptiveReader {
    inner: AdaptiveReaderType,
}

enum AdaptiveReaderType {
    Bounded(Box<dyn StorageReader>),
    Unbounded(Box<dyn StorageReader>),
}

impl Read for AdaptiveReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.inner {
            AdaptiveReaderType::Bounded(r) => r.read(buf),
            AdaptiveReaderType::Unbounded(r) => r.read(buf),
        }
    }
}

impl Seek for AdaptiveReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match &mut self.inner {
            AdaptiveReaderType::Bounded(r) => r.seek(pos),
            AdaptiveReaderType::Unbounded(r) => r.seek(pos),
        }
    }
}

/// Writer that adaptively uses either bounded or unbounded storage
pub struct AdaptiveWriter {
    inner: AdaptiveWriterType,
}

enum AdaptiveWriterType {
    Bounded(Box<dyn StorageWriter>),
    Unbounded(Box<dyn StorageWriter>),
}

impl Write for AdaptiveWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match &mut self.inner {
            AdaptiveWriterType::Bounded(w) => w.write(buf),
            AdaptiveWriterType::Unbounded(w) => w.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match &mut self.inner {
            AdaptiveWriterType::Bounded(w) => w.flush(),
            AdaptiveWriterType::Unbounded(w) => w.flush(),
        }
    }
}

impl Seek for AdaptiveWriter {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match &mut self.inner {
            AdaptiveWriterType::Bounded(w) => w.seek(pos),
            AdaptiveWriterType::Unbounded(w) => w.seek(pos),
        }
    }
}
