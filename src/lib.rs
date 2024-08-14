#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![forbid(clippy::unwrap_used)]
#![deny(rustdoc::broken_intra_doc_links)]
#![warn(clippy::semicolon_if_nothing_returned)]
#![warn(clippy::doc_markdown)]
#![warn(clippy::default_trait_access)]
#![warn(clippy::ignored_unit_patterns)]
#![warn(clippy::semicolon_if_nothing_returned)]
#![warn(clippy::missing_fields_in_debug)]
#![warn(clippy::use_self)]
#![warn(missing_debug_implementations)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc = include_str!("../README.md")]

use std::fmt::Debug;
use std::future::{self, Future};
use std::io::{self, Read, Seek, SeekFrom};
use std::ops::Range;
use std::time::Duration;

use educe::Educe;
use source::{Source, SourceHandle, SourceStream};
use storage::StorageProvider;
use tap::{Tap, TapFallible};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, instrument, trace};

#[cfg(feature = "http")]
pub mod http;
#[cfg(feature = "open-dal")]
pub mod open_dal;
pub mod source;
pub mod storage;

/// Current phase of the download for use during a progress callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum StreamPhase {
    /// Stream is currently in a prefetch state.
    #[non_exhaustive]
    Prefetching {
        /// Current prefetch target.
        target: u64,
        /// Size of the most recently downloaded chunk.
        chunk_size: usize,
    },
    /// Stream is currently in a downloading state.
    #[non_exhaustive]
    Downloading {
        /// Size of the most recently downloaded chunk.
        chunk_size: usize,
    },
    /// Stream has finished downloading.
    Complete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
/// State of the stream for use during a progress callback.
pub struct StreamState {
    /// Current position of the stream.
    pub current_position: u64,
    /// Time elapsed since download start.
    pub elapsed: Duration,
    /// Current phase of the download.
    pub phase: StreamPhase,
    /// Current chunk of the stream being downloaded.
    pub current_chunk: Range<u64>,
}

type CallbackFn<S> = Box<dyn FnMut(&S, StreamState) + Send + Sync>;

/// Settings to configure the stream behavior.
#[derive(Educe)]
#[educe(Debug, PartialEq, Eq)]
pub struct Settings<S> {
    prefetch_bytes: u64,
    seek_buffer_size: usize,
    #[educe(Debug = false, PartialEq = false)]
    on_progress: Option<CallbackFn<S>>,
}

impl<S> Default for Settings<S> {
    fn default() -> Self {
        Self {
            prefetch_bytes: 256 * 1024,
            seek_buffer_size: 128,
            on_progress: None,
        }
    }
}

impl<S> Settings<S> {
    /// How many bytes to download from the stream before allowing read requests.
    /// This is used to create a buffer between the read position and the stream position
    /// and prevent stuttering.
    ///
    /// The default value is 256 kilobytes.
    #[must_use]
    pub fn prefetch_bytes(self, prefetch_bytes: u64) -> Self {
        Self {
            prefetch_bytes,
            ..self
        }
    }

    /// The internal buffer size used to process seek requests.
    /// You shouldn't need to mess with this unless your application performs a lot of seek
    /// requests and you're seeing error messages from the buffer filling up.
    ///
    /// The default value is 128.
    #[must_use]
    pub fn seek_buffer_size(self, seek_buffer_size: usize) -> Self {
        Self {
            seek_buffer_size,
            ..self
        }
    }

    /// Attach a callback function that will be called when a new chunk of the stream is processed.
    ///
    /// # Example
    ///
    /// ```
    /// use reqwest::Client;
    /// use stream_download::http::HttpStream;
    /// use stream_download::source::SourceStream;
    /// use stream_download::Settings;
    ///
    /// let settings = Settings::default();
    /// settings.on_progress(|stream: &HttpStream<Client>, state| {
    ///     let progress = state.current_position as f32 / stream.content_length().unwrap() as f32;
    ///     println!("progress: {}%", progress * 100.0);
    /// });
    /// ```
    #[must_use]
    pub fn on_progress<F>(mut self, f: F) -> Self
    where
        F: Fn(&S, StreamState) + Send + Sync + 'static,
    {
        self.on_progress = Some(Box::new(f));
        self
    }

    /// Retrieves the configured prefetch bytes
    pub const fn get_prefetch_bytes(&self) -> u64 {
        self.prefetch_bytes
    }

    /// Retrieves the configured seek buffer size
    pub const fn get_seek_buffer_size(&self) -> usize {
        self.seek_buffer_size
    }
}

/// Represents content streamed from a remote source.
/// This struct implements [read](https://doc.rust-lang.org/stable/std/io/trait.Read.html)
/// and [seek](https://doc.rust-lang.org/stable/std/io/trait.Seek.html)
/// so it can be used as a generic source for libraries and applications that operate on these
/// traits. On creation, an async task is spawned that will immediately start to download the remote
/// content.
///
/// Any read attempts that request part of the stream that hasn't been downloaded yet will block
/// until the requested portion is reached. Any seek attempts that meet the same criteria will
/// result in additional request to restart the stream download from the seek point.
///
/// If the stream download hasn't completed when this struct is dropped, the task will be cancelled.
#[derive(Debug)]
pub struct StreamDownload<P: StorageProvider> {
    output_reader: P::Reader,
    handle: SourceHandle,
    download_task_cancellation_token: CancellationToken,
}

impl<P: StorageProvider> StreamDownload<P> {
    #[cfg(feature = "reqwest")]
    /// Creates a new [`StreamDownload`] that accesses an HTTP resource at the given URL.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::error::Error;
    /// use std::io::Read;
    /// use std::result::Result;
    ///
    /// use stream_download::storage::temp::TempStorageProvider;
    /// use stream_download::{Settings, StreamDownload};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let mut reader = StreamDownload::new_http(
    ///         "https://some-cool-url.com/some-file.mp3".parse()?,
    ///         TempStorageProvider::default(),
    ///         Settings::default(),
    ///     )
    ///     .await?;
    ///
    ///     let mut buf = Vec::new();
    ///     reader.read_to_end(&mut buf)?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn new_http(
        url: ::reqwest::Url,
        storage_provider: P,
        settings: Settings<http::HttpStream<::reqwest::Client>>,
    ) -> io::Result<Self> {
        Self::new(url, storage_provider, settings).await
    }

    #[cfg(feature = "open-dal")]
    /// Creates a new [`StreamDownload`] that uses an `OpenDAL` resource.
    /// See the [`open_dal`] documentation for more details.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::error::Error;
    /// use std::io::Read;
    /// use std::result::Result;
    ///
    /// use opendal::{services, Operator};
    /// use stream_download::open_dal::OpenDalStreamParams;
    /// use stream_download::storage::temp::TempStorageProvider;
    /// use stream_download::{Settings, StreamDownload};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let mut builder = services::S3::default();
    ///     builder
    ///         .region("us-east-1")
    ///         .access_key_id("test")
    ///         .secret_access_key("test")
    ///         .bucket("my-bucket");
    ///     let operator = Operator::new(builder)?.finish();
    ///
    ///     let mut reader = StreamDownload::new_open_dal(
    ///         OpenDalStreamParams::new(operator, "some-object-key"),
    ///         TempStorageProvider::default(),
    ///         Settings::default(),
    ///     )
    ///     .await?;
    ///
    ///     let mut buf = Vec::new();
    ///     reader.read_to_end(&mut buf)?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn new_open_dal(
        params: open_dal::OpenDalStreamParams,
        storage_provider: P,
        settings: Settings<open_dal::OpenDalStream>,
    ) -> io::Result<Self> {
        Self::new(params, storage_provider, settings).await
    }

    /// Creates a new [`StreamDownload`] that accesses a remote resource at the given URL.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::error::Error;
    /// use std::io::Read;
    /// use std::result::Result;
    ///
    /// use reqwest::Client;
    /// use stream_download::http::HttpStream;
    /// use stream_download::storage::temp::TempStorageProvider;
    /// use stream_download::{Settings, StreamDownload};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let mut reader = StreamDownload::new::<HttpStream<Client>>(
    ///         "https://some-cool-url.com/some-file.mp3".parse()?,
    ///         TempStorageProvider::default(),
    ///         Settings::default(),
    ///     )
    ///     .await?;
    ///
    ///     let mut buf = Vec::new();
    ///     reader.read_to_end(&mut buf)?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn new<S: SourceStream>(
        url: S::Params,
        storage_provider: P,
        settings: Settings<S>,
    ) -> io::Result<Self> {
        Self::from_make_stream(move || S::create(url), storage_provider, settings).await
    }

    /// Creates a new [`StreamDownload`] from a [`SourceStream`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::error::Error;
    /// use std::io::Read;
    /// use std::result::Result;
    ///
    /// use reqwest::Client;
    /// use stream_download::http::HttpStream;
    /// use stream_download::storage::temp::TempStorageProvider;
    /// use stream_download::{Settings, StreamDownload};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let stream = HttpStream::new(
    ///         Client::new(),
    ///         "https://some-cool-url.com/some-file.mp3".parse()?,
    ///     )
    ///     .await?;
    ///
    ///     let mut reader = StreamDownload::from_stream(
    ///         stream,
    ///         TempStorageProvider::default(),
    ///         Settings::default(),
    ///     )
    ///     .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn from_stream<S: SourceStream>(
        stream: S,
        storage_provider: P,
        settings: Settings<S>,
    ) -> Result<Self, io::Error> {
        Self::from_make_stream(
            move || future::ready(Ok(stream)),
            storage_provider,
            settings,
        )
        .await
    }

    /// Cancels the background task that's downloading the stream content.
    /// This has no effect if the download is already completed.
    pub fn cancel_download(&self) {
        self.download_task_cancellation_token.cancel();
    }

    /// Get a cancellation token
    pub fn get_cancellation_token(&self) -> CancellationToken {
        return self.download_task_cancellation_token.clone();
    }

    async fn from_make_stream<S, F, Fut>(
        make_stream: F,
        storage_provider: P,
        settings: Settings<S>,
    ) -> Result<Self, io::Error>
    where
        S: SourceStream,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = io::Result<S>> + Send,
    {
        let stream = make_stream().await.wrap_err("error creating stream")?;
        let content_length = stream.content_length();
        let (reader, writer) = storage_provider.into_reader_writer(content_length)?;
        let source = Source::new(writer, content_length, settings);
        let handle = source.source_handle();
        let cancellation_token = CancellationToken::new();
        let cancellation_token_ = cancellation_token.clone();

        tokio::spawn(async move {
            source
                .download(stream, cancellation_token_)
                .await
                .tap_err(|e| error!("Error downloading stream: {e}"))?;
            debug!("download task finished");
            Ok::<_, io::Error>(())
        });

        Ok(Self {
            output_reader: reader,
            handle,
            download_task_cancellation_token: cancellation_token,
        })
    }

    fn get_absolute_seek_position(&mut self, relative_position: SeekFrom) -> io::Result<u64> {
        Ok(match relative_position {
            SeekFrom::Start(position) => {
                debug!(seek_position = position, "seeking from start");
                position
            }
            SeekFrom::End(position) => {
                debug!(seek_position = position, "seeking from end");
                if let Some(length) = self.handle.content_length() {
                    (length as i64 - position) as u64
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "cannot seek from end when content length is unknown",
                    ));
                }
            }
            SeekFrom::Current(position) => {
                debug!(seek_position = position, "seeking from current position");
                (self.output_reader.stream_position()? as i64 + position) as u64
            }
        })
    }
}

impl<P: StorageProvider> Drop for StreamDownload<P> {
    fn drop(&mut self) {
        self.cancel_download();
    }
}

impl<P: StorageProvider> Read for StreamDownload<P> {
    #[instrument(skip_all)]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        trace!(buffer_length = buf.len(), "read requested");
        let stream_position = self.output_reader.stream_position()?;
        let requested_position = stream_position + buf.len() as u64;
        trace!(
            current_position = stream_position,
            requested_position = requested_position
        );

        if let Some(closest_set) = self.handle.get_downloaded_at_position(stream_position) {
            trace!(
                downloaded_range = format!("{closest_set:?}"),
                "current position already downloaded"
            );
            if closest_set.end >= requested_position {
                return self.output_reader.read(buf).tap(|l| {
                    trace!(
                        read_length = format!("{l:?}"),
                        "requested position already downloaded, returning read"
                    );
                });
            }
            debug!("requested position not yet downloaded");
        } else {
            debug!("stream position not yet downloaded");
        }

        self.handle.request_position(requested_position);
        debug!(
            requested_position = requested_position,
            "waiting for requested position"
        );
        self.handle.wait_for_requested_position();
        debug!(
            current_position = stream_position,
            requested_position = requested_position,
            output_stream_position = self.output_reader.stream_position()?,
            "reached requested position"
        );

        self.output_reader
            .read(buf)
            .tap(|l| debug!(read_length = format!("{l:?}"), "returning read"))
    }
}

impl<P: StorageProvider> Seek for StreamDownload<P> {
    #[instrument(skip(self))]
    fn seek(&mut self, relative_position: SeekFrom) -> io::Result<u64> {
        let absolute_seek_position = self.get_absolute_seek_position(relative_position)?;

        debug!(absolute_seek_position, "absolute seek position");
        if let Some(closest_set) = self
            .handle
            .get_downloaded_at_position(absolute_seek_position)
        {
            debug!(
                downloaded_range = format!("{closest_set:?}"),
                "seek position already downloaded"
            );
            return self
                .output_reader
                .seek(SeekFrom::Start(absolute_seek_position))
                .tap(|p| debug!(position = format!("{p:?}"), "returning seek position"));
        }

        self.handle.request_position(absolute_seek_position);
        self.handle.seek(absolute_seek_position);
        debug!(
            requested_position = absolute_seek_position,
            "waiting for requested position"
        );
        self.handle.wait_for_requested_position();
        debug!("reached seek position");

        self.output_reader
            .seek(SeekFrom::Start(absolute_seek_position))
            .tap(|p| debug!(position = format!("{p:?}"), "returning seek position"))
    }
}

pub(crate) trait WrapIoResult {
    fn wrap_err(self, msg: &str) -> Self;
}

impl<T> WrapIoResult for io::Result<T> {
    fn wrap_err(self, msg: &str) -> Self {
        if let Err(e) = self {
            Err(io::Error::new(e.kind(), format!("{msg}: {e}")))
        } else {
            self
        }
    }
}
