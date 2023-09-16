//! Configurable implementations for the buffer's storage layer.
//! Pre-configured implementations are available for memory and temporary file-based storage.
use std::io::{self, Read, Seek, Write};

pub mod adaptive;
pub mod bounded;
pub mod memory;
#[cfg(feature = "temp-storage")]
pub mod temp;

/// Creates a [StorageReader] based on the content length returned from the
/// [SourceStream](crate::source::SourceStream).And construct a writable handle.
/// The reader and writer must track their position in the stream independently.

pub trait StorageProvider: Clone + Send {
    /// Source used to read from the underlying storage.
    type Reader: StorageReader;
    /// Handle that can write to the underlying storage.
    type Writer: StorageWriter;

    /// Builds the reader with the specified content length.
    fn create_reader(&self, content_length: Option<u64>) -> io::Result<Self::Reader>;

    /// Returns a handle that can write to the underlying storage.
    fn writer(&self) -> io::Result<Self::Writer>;

    // Turn the provider into a reader and writer.
    // TODO provide two methods instead of having this argument
//     fn into_reader_writer(self, content_length: Option<u64>) -> io::Result<(Self::Reader, Self::Writer)>;
}

/// Trait used to read from a storage layer
pub trait StorageReader: Read + Seek + Send {}

/// Handle for writing to the underlying storage layer.
pub trait StorageWriter: Write + Seek + Send + 'static {}

impl<T> StorageWriter for T where T: Write + Seek + Send + 'static {}
