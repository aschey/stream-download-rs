//! Configurable implementations for the buffer's storage layer.
//! Pre-configured implementations are available for memory and temporary file-based storage.
use std::io::{self, Read, Seek, Write};

pub mod adaptive;
pub mod bounded;
pub mod memory;
pub mod temp;

/// Creates a [StorageReader] based on the content length returned from the
/// [SourceStream](crate::source::SourceStream).
pub trait StorageProvider: Clone + Send {
    /// Source used to read from the underlying storage.
    type Reader: StorageReader;
    /// Builds the reader with the specified content length.
    fn create_reader(&self, content_length: Option<u64>) -> io::Result<Self::Reader>;
}

/// Trait used to read from a storage layer and construct a writable handle.
/// The reader and writer must track their position in the stream independently.
pub trait StorageReader: Read + Seek + Send {
    /// Handle that can write to the underlying storage.
    type Writer: StorageWriter;

    /// Returns a handle that can write to the underlying storage.
    fn writer(&self) -> io::Result<Self::Writer>;
}

/// Handle for writing to the underlying storage layer.
pub trait StorageWriter: Write + Seek + Send + 'static {}

impl<T> StorageWriter for T where T: Write + Seek + Send + 'static {}
