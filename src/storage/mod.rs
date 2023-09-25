//! Configurable implementations for the buffer's storage layer.
//! Pre-configured implementations are available for memory and temporary file-based storage.
use std::io::{self, Read, Seek, Write};

use rangemap::RangeSet;

pub mod adaptive;
pub mod bounded;
pub mod memory;
#[cfg(feature = "temp-storage")]
pub mod temp;
#[cfg(feature = "unstable-permanent-storage")]
pub mod permanent;

/// Creates a [`StorageReader`] and [`StorageWriter`] based on the content
/// length returned from the [`SourceStream`](crate::source::SourceStream).
/// The reader and writer must track their position in the stream independently.
pub trait StorageProvider: Clone + Send {
    /// Source used to read from the underlying storage.
    type Reader: StorageReader;
    /// Handle that can write to the underlying storage.
    type Writer: StorageWriter;

    /// Turn the provider into a reader and writer.
    fn into_reader_writer(
        self,
        content_length: Option<u64>,
    ) -> io::Result<(Self::Reader, Self::Writer)>;
}

/// Trait used to read from a storage layer
pub trait StorageReader: Read + Seek + Send {}

impl<T> StorageReader for T where T: Read + Seek + Send {}

/// Handle for writing to the underlying storage layer.
pub trait StorageWriter: Write + Seek + Send + 'static {
    /// If the underlying storage can resume a download return progress
    /// already made
    fn downloaded(&self) -> RangeSet<u64> {
        RangeSet::new()
    }
}
