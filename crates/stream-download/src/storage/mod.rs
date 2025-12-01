//! Configurable implementations for the buffer's storage layer.
//! Pre-configured implementations are available for memory and temporary file-based storage.

use std::io::{self, Read, Seek, Write};

pub mod adaptive;
pub mod bounded;
pub mod memory;
#[cfg(feature = "temp-storage")]
pub mod temp;

/// Represents a content length that can change during processing.
///
/// This structure holds:
/// - `reported`: The length of the content as reported by the server (e.g., Content-Range header).
/// - `gathered`: The actual length of the content after processing (e.g., decryption,
///   decompression), or `None` if the final value is not yet known (still being calculated or
///   accumulated).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DynamicLength {
    /// The length of the content as reported by the server, for example, via Content-Range header.
    pub reported: u64,
    /// The actual content length gathered after all processing.
    /// This remains `None` until the entire content is processed and its final length determined.
    pub gathered: Option<u64>,
}

/// Describes the possible states for the length of a stream or content resource.
///
/// `ContentLength` distinguishes between content with a fixed, knowable size,
/// content whose final size is determined only after processing,
/// and content where the length is unknown.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContentLength {
    /// Static content length that remains constant throughout processing.
    ///
    /// Contains the exact number of bytes in the content as reported by the server or known
    /// beforehand. This value does not change during decryption, decompression, or any form of
    /// data transformation.
    ///
    /// Examples:
    /// - Regular file downloads
    /// - Static media files with a fixed size
    /// - HTTP responses with a `Content-Length` header
    Static(u64),

    /// Dynamic content length that may change during reading or processing.
    ///
    /// The [`DynamicLength`] structure stores both the initially reported length from the server,
    /// and the accumulated "gathered" length as chunks are processed.
    /// `gathered` may temporarily be `None` until the content is fully processed
    /// (for example, for decrypted, transformed, or recomposed streams).
    ///
    /// Examples:
    /// - Encrypted streams/files (where decrypted size may differ)
    /// - Compressed streams/files (expanding during decompression)
    /// - Media content assembled from segments
    Dynamic(DynamicLength),

    /// Unknown content length.
    ///
    /// Used when the server does not provide content length information
    /// and it cannot be determined up front.
    /// This is typical for live streaming, HTTP chunked transfer, or similar scenarios.
    ///
    /// Examples:
    /// - Live streams without a defined end
    /// - HTTP chunked responses
    /// - Server responses lacking any length information
    Unknown,
}

impl ContentLength {
    /// Creates a new static content length.
    ///
    /// This is typically used for media content that has a fixed size.
    pub fn new_static(value: u64) -> Self {
        Self::Static(value)
    }

    /// Creates a new dynamic content length.
    ///
    /// This is typically used for media content that has an unknown size.
    pub fn new_dynamic(value: DynamicLength) -> Self {
        Self::Dynamic(value)
    }

    /// Creates a new unknown content length.
    ///
    /// This is typically used for media content that has an unknown size.
    pub fn new_unknown() -> Self {
        Self::Unknown
    }

    /// Returns the current value of the content length.
    ///
    /// For static lengths, this is the exact length.
    /// For dynamic lengths, this is the current length if known or the value
    /// returned by the server before starting the data download.
    /// For unknown lengths, this is always `None`.
    pub fn current_value(&self) -> Option<u64> {
        match self {
            Self::Static(len) => Some(*len),
            Self::Dynamic(len) => Some(len.gathered.unwrap_or(len.reported)),
            Self::Unknown => None,
        }
    }
}

impl From<u64> for ContentLength {
    fn from(value: u64) -> Self {
        Self::new_static(value)
    }
}

impl From<DynamicLength> for ContentLength {
    fn from(value: DynamicLength) -> Self {
        Self::new_dynamic(value)
    }
}

impl From<ContentLength> for Option<u64> {
    fn from(val: ContentLength) -> Self {
        val.current_value()
    }
}

impl PartialEq<u64> for ContentLength {
    fn eq(&self, other: &u64) -> bool {
        match self {
            Self::Static(len) => len == other,
            Self::Dynamic(len) => len.gathered.unwrap_or(len.reported) == *other,
            Self::Unknown => false,
        }
    }
}

impl PartialOrd<u64> for ContentLength {
    fn partial_cmp(&self, other: &u64) -> Option<std::cmp::Ordering> {
        match self {
            Self::Static(len) => len.partial_cmp(other),
            Self::Dynamic(len) => len.gathered.unwrap_or(len.reported).partial_cmp(other),
            Self::Unknown => None,
        }
    }
}

/// Creates a [`StorageReader`] and [`StorageWriter`] based on the content
/// length returned from the [`SourceStream`](crate::source::SourceStream).
/// The reader and writer must track their position in the stream independently.
pub trait StorageProvider: Send {
    /// Source used to read from the underlying storage.
    type Reader: StorageReader;
    /// Handle that can write to the underlying storage.
    type Writer: StorageWriter;

    /// Turn the provider into a reader and writer.
    fn into_reader_writer(
        self,
        content_length: ContentLength,
    ) -> io::Result<(Self::Reader, Self::Writer)>;

    /// Returns the maximum number of bytes this provider can hold at a time.
    fn max_capacity(&self) -> Option<usize> {
        None
    }
}

/// Trait used to read from a storage layer
pub trait StorageReader: Read + Seek + Send {}

impl<T> StorageReader for T where T: Read + Seek + Send {}

/// Handle for writing to the underlying storage layer.
pub trait StorageWriter: Write + Seek + Send + 'static {}

impl<T> StorageWriter for T where T: Write + Seek + Send + 'static {}
