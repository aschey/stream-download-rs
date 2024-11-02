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
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use educe::Educe;
use source::{DecodeError, Source, SourceHandle, SourceStream};
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

type ProgressFn<S> = Box<dyn FnMut(&S, StreamState, &CancellationToken) + Send + Sync>;

type ReconnectFn<S> = Box<dyn FnMut(&S, &CancellationToken) + Send + Sync>;

/// Settings to configure the stream behavior.
#[derive(Educe)]
#[educe(Debug, PartialEq, Eq)]
pub struct Settings<S> {
    prefetch_bytes: u64,
    seek_buffer_size: usize,
    retry_timeout: Duration,
    #[educe(Debug = false, PartialEq = false)]
    on_progress: Option<ProgressFn<S>>,
    #[educe(Debug = false, PartialEq = false)]
    on_reconnect: Option<ReconnectFn<S>>,
}

impl<S> Default for Settings<S> {
    fn default() -> Self {
        Self {
            prefetch_bytes: 256 * 1024,
            seek_buffer_size: 128,
            retry_timeout: Duration::from_secs(5),
            on_progress: None,
            on_reconnect: None,
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

    /// If there is no new data for a duration greater than this timeout, we will attempt to
    /// reconnect to the server.
    ///  
    /// This timeout is designed to help streams recover during temporary network failures,
    /// but you may need to increase this if you're seeing warnings about timeouts in the logs
    /// under normal network conditions.
    ///
    /// The default value is 5 seconds.
    #[must_use]
    pub fn retry_timeout(self, retry_timeout: Duration) -> Self {
        Self {
            retry_timeout,
            ..self
        }
    }

    /// Attach a callback function that will be called when a new chunk of the stream is processed.
    /// The provided [`CancellationToken`] can be used to immediately cancel the stream.
    ///
    /// # Example
    ///
    /// ```
    /// use reqwest::Client;
    /// use stream_download::Settings;
    /// use stream_download::http::HttpStream;
    /// use stream_download::source::SourceStream;
    ///
    /// let settings = Settings::default();
    /// settings.on_progress(|stream: &HttpStream<Client>, state, _| {
    ///     let progress = state.current_position as f32 / stream.content_length().unwrap() as f32;
    ///     println!("progress: {}%", progress * 100.0);
    /// });
    /// ```
    #[must_use]
    pub fn on_progress<F>(mut self, f: F) -> Self
    where
        F: FnMut(&S, StreamState, &CancellationToken) + Send + Sync + 'static,
    {
        self.on_progress = Some(Box::new(f));
        self
    }

    /// Attach a callback function that will be called when the stream reconnects after a failure.
    /// The provided [`CancellationToken`] can be used to immediately cancel the stream.
    pub fn on_reconnect<F>(mut self, f: F) -> Self
    where
        F: FnMut(&S, &CancellationToken) + Send + Sync + 'static,
    {
        self.on_reconnect = Some(Box::new(f));
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
    download_status: DownloadStatus,
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
    /// use std::io::{self, Read};
    /// use std::result::Result;
    ///
    /// use stream_download::source::DecodeError;
    /// use stream_download::storage::temp::TempStorageProvider;
    /// use stream_download::{Settings, StreamDownload};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let mut reader = match StreamDownload::new_http(
    ///         "https://some-cool-url.com/some-file.mp3".parse()?,
    ///         TempStorageProvider::default(),
    ///         Settings::default(),
    ///     )
    ///     .await
    ///     {
    ///         Ok(reader) => reader,
    ///         Err(e) => return Err(e.decode_error().await)?,
    ///     };
    ///
    ///     tokio::task::spawn_blocking(move || {
    ///         let mut buf = Vec::new();
    ///         reader.read_to_end(&mut buf)?;
    ///         Ok::<_, io::Error>(())
    ///     })
    ///     .await??;
    ///     Ok(())
    /// }
    /// ```
    pub async fn new_http(
        url: ::reqwest::Url,
        storage_provider: P,
        settings: Settings<http::HttpStream<::reqwest::Client>>,
    ) -> Result<Self, StreamInitializationError<http::HttpStream<::reqwest::Client>>> {
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
    /// use std::io::{self, Read};
    /// use std::result::Result;
    ///
    /// use opendal::{Operator, services};
    /// use stream_download::open_dal::OpenDalStreamParams;
    /// use stream_download::storage::temp::TempStorageProvider;
    /// use stream_download::{Settings, StreamDownload};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let mut builder = services::S3::default()
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
    ///     tokio::task::spawn_blocking(move || {
    ///         let mut buf = Vec::new();
    ///         reader.read_to_end(&mut buf)?;
    ///         Ok::<_, io::Error>(())
    ///     })
    ///     .await??;
    ///     Ok(())
    /// }
    /// ```
    pub async fn new_open_dal(
        params: open_dal::OpenDalStreamParams,
        storage_provider: P,
        settings: Settings<open_dal::OpenDalStream>,
    ) -> Result<Self, StreamInitializationError<open_dal::OpenDalStream>> {
        Self::new(params, storage_provider, settings).await
    }

    /// Creates a new [`StreamDownload`] that accesses a remote resource at the given URL.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::error::Error;
    /// use std::io::{self, Read};
    /// use std::result::Result;
    ///
    /// use reqwest::Client;
    /// use stream_download::http::HttpStream;
    /// use stream_download::storage::temp::TempStorageProvider;
    /// use stream_download::{Settings, StreamDownload};
    ///
    /// use crate::stream_download::source::DecodeError;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let mut reader = match StreamDownload::new::<HttpStream<Client>>(
    ///         "https://some-cool-url.com/some-file.mp3".parse()?,
    ///         TempStorageProvider::default(),
    ///         Settings::default(),
    ///     )
    ///     .await
    ///     {
    ///         Ok(reader) => reader,
    ///         Err(e) => return Err(e.decode_error().await)?,
    ///     };
    ///
    ///     tokio::task::spawn_blocking(move || {
    ///         let mut buf = Vec::new();
    ///         reader.read_to_end(&mut buf)?;
    ///         Ok::<_, io::Error>(())
    ///     })
    ///     .await??;
    ///     Ok(())
    /// }
    /// ```
    pub async fn new<S>(
        url: S::Params,
        storage_provider: P,
        settings: Settings<S>,
    ) -> Result<Self, StreamInitializationError<S>>
    where
        S: SourceStream,
        S::Error: Debug + Send,
    {
        Self::from_create_stream(move || S::create(url), storage_provider, settings).await
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
    /// use crate::stream_download::source::DecodeError;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let stream = HttpStream::new(
    ///         Client::new(),
    ///         "https://some-cool-url.com/some-file.mp3".parse()?,
    ///     )
    ///     .await?;
    ///
    ///     let mut reader = match StreamDownload::from_stream(
    ///         stream,
    ///         TempStorageProvider::default(),
    ///         Settings::default(),
    ///     )
    ///     .await
    ///     {
    ///         Ok(reader) => reader,
    ///         Err(e) => Err(e.decode_error().await)?,
    ///     };
    ///     Ok(())
    /// }
    /// ```
    pub async fn from_stream<S>(
        stream: S,
        storage_provider: P,
        settings: Settings<S>,
    ) -> Result<Self, StreamInitializationError<S>>
    where
        S: SourceStream,
        S::Error: Debug + Send,
    {
        Self::from_create_stream(
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

    /// Get the [`CancellationToken`] for the download task.
    pub fn get_cancellation_token(&self) -> CancellationToken {
        self.download_task_cancellation_token.clone()
    }

    async fn from_create_stream<S, F, Fut>(
        create_stream: F,
        storage_provider: P,
        settings: Settings<S>,
    ) -> Result<Self, StreamInitializationError<S>>
    where
        S: SourceStream,
        S::Error: Debug + Send,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<S, S::StreamCreationError>> + Send,
    {
        let stream = create_stream()
            .await
            .map_err(StreamInitializationError::StreamCreationFailure)?;
        let content_length = stream.content_length();
        let (reader, writer) = storage_provider
            .into_reader_writer(content_length)
            .map_err(StreamInitializationError::StorageCreationFailure)?;
        let cancellation_token = CancellationToken::new();
        let mut source = Source::new(writer, content_length, settings, cancellation_token.clone());
        let handle = source.source_handle();

        let download_status = DownloadStatus::default();
        tokio::spawn({
            let download_status = download_status.clone();
            async move {
                if source
                    .download(stream)
                    .await
                    .tap_err(|e| error!("Error downloading stream: {e}"))
                    .is_err()
                {
                    download_status.set_failed();
                    source.signal_download_complete();
                }
                debug!("download task finished");
            }
        });

        Ok(Self {
            output_reader: reader,
            handle,
            download_status,
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
                    (length as i64 + position) as u64
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

    fn handle_read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let res = self.output_reader.read(buf).tap(|l| {
            trace!(read_length = format!("{l:?}"), "returning read");
        });
        self.handle.notify_read();
        res
    }

    fn check_for_failure(&self) -> io::Result<()> {
        if self.download_status.is_failed() {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "stream failed to download",
            ))
        } else {
            Ok(())
        }
    }
}

/// Error returned when initializing a stream.
#[derive(thiserror::Error, Educe)]
#[educe(Debug)]
pub enum StreamInitializationError<S: SourceStream> {
    /// Storage creation failure.
    #[error("Storage creation failure: {0}")]
    StorageCreationFailure(io::Error),
    /// Stream creation failure.
    #[error("Stream creation failure: {0}")]
    StreamCreationFailure(<S as SourceStream>::StreamCreationError),
}

impl<S: SourceStream> DecodeError for StreamInitializationError<S> {
    async fn decode_error(self) -> String {
        match self {
            this @ Self::StorageCreationFailure(_) => this.to_string(),
            Self::StreamCreationFailure(e) => e.decode_error().await,
        }
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
        self.check_for_failure()?;

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
                debug!("requested position already downloaded");
                return self.handle_read(buf);
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
        self.handle.notify_waiting();
        self.handle.wait_for_requested_position();
        self.check_for_failure()?;
        debug!(
            current_position = stream_position,
            requested_position = requested_position,
            output_stream_position = self.output_reader.stream_position()?,
            "reached requested position"
        );

        self.handle_read(buf)
    }
}

impl<P: StorageProvider> Seek for StreamDownload<P> {
    #[instrument(skip(self))]
    fn seek(&mut self, relative_position: SeekFrom) -> io::Result<u64> {
        self.check_for_failure()?;

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
        self.check_for_failure()?;
        debug!("reached seek position");

        self.output_reader
            .seek(SeekFrom::Start(absolute_seek_position))
            .tap(|p| debug!(position = format!("{p:?}"), "returning seek position"))
    }
}

#[derive(Default, Clone, Debug)]
struct DownloadStatus(Arc<AtomicBool>);

impl DownloadStatus {
    fn set_failed(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    fn is_failed(&self) -> bool {
        self.0.load(Ordering::SeqCst)
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
