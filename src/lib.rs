#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![forbid(clippy::unwrap_used)]
#![deny(rustdoc::broken_intra_doc_links)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc = include_str!("../README.md")]

use std::future::{self, Future};
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::thread;

use source::{Source, SourceHandle, SourceStream};
use tap::{Tap, TapFallible};
use tempfile::NamedTempFile;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, instrument, trace};

#[cfg(feature = "http")]
pub mod http;
#[cfg(feature = "reqwest")]
pub mod reqwest;
pub mod source;

/// Settings to configure the stream behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    prefetch_bytes: u64,
    download_dir: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            prefetch_bytes: 256 * 1024,
            download_dir: None,
        }
    }
}

impl Settings {
    /// How many bytes to download from the stream before allowing read requests.
    /// This is used to create a buffer between the read position and the stream position
    /// and prevent stuttering.
    /// The default value is 256 kilobytes.
    pub fn prefetch_bytes(self, prefetch_bytes: u64) -> Self {
        Self {
            prefetch_bytes,
            ..self
        }
    }

    /// Set the directory used to download the streamed content.
    /// Temporary files will be cleaned up when the stream is dropped.
    /// By default this uses the OS-specific temporary directory.
    pub fn download_dir(self, download_dir: impl Into<PathBuf>) -> Self {
        Self {
            download_dir: Some(download_dir.into()),
            ..self
        }
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
pub struct StreamDownload {
    output_reader: BufReader<NamedTempFile>,
    handle: SourceHandle,
    download_task_cancellation_token: CancellationToken,
}

impl StreamDownload {
    #[cfg(feature = "reqwest")]
    /// Creates a new [StreamDownload] that accesses an HTTP resource at the given URL.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::error::Error;
    /// use std::io::Read;
    /// use std::result::Result;
    ///
    /// use stream_download::{Settings, StreamDownload};
    ///
    /// fn main() -> Result<(), Box<dyn Error>> {
    ///     let mut reader = StreamDownload::new_http(
    ///         "https://some-cool-url.com/some-file.mp3".parse()?,
    ///         Settings::default(),
    ///     )?;
    ///
    ///     let mut buf = Vec::new();
    ///     reader.read_to_end(&mut buf)?;
    ///     Ok(())
    /// }
    /// ```
    pub fn new_http(url: ::reqwest::Url, settings: Settings) -> io::Result<Self> {
        Self::new::<http::HttpStream<::reqwest::Client>>(url, settings)
    }

    /// Creates a new [StreamDownload] that accesses a remote resource at the given URL.
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
    /// use stream_download::{Settings, StreamDownload};
    ///
    /// fn main() -> Result<(), Box<dyn Error>> {
    ///     let mut reader = StreamDownload::new::<HttpStream<Client>>(
    ///         "https://some-cool-url.com/some-file.mp3".parse()?,
    ///         Settings::default(),
    ///     )?;
    ///
    ///     let mut buf = Vec::new();
    ///     reader.read_to_end(&mut buf)?;
    ///     Ok(())
    /// }
    /// ```
    pub fn new<S: SourceStream>(url: S::Url, settings: Settings) -> io::Result<Self> {
        Self::from_make_stream(move || S::create(url), settings)
    }

    /// Creates a new [StreamDownload] from a [SourceStream].
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
    ///     let mut reader = StreamDownload::from_stream(stream, Settings::default())?;
    ///     Ok(())
    /// }
    /// ```
    pub fn from_stream<S: SourceStream>(stream: S, settings: Settings) -> Result<Self, io::Error> {
        Self::from_make_stream(move || future::ready(Ok(stream)), settings)
    }

    /// Cancels the background task that's downloading the stream content.
    /// This has no effect if the download is already completed.
    pub fn cancel_download(&self) {
        self.download_task_cancellation_token.cancel();
    }

    fn from_make_stream<S, F, Fut>(make_stream: F, settings: Settings) -> Result<Self, io::Error>
    where
        S: SourceStream,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = io::Result<S>> + Send,
    {
        let tempfile = if let Some(temp_dir) = &settings.download_dir {
            NamedTempFile::new_in(temp_dir)?
        } else {
            NamedTempFile::new()?
        };

        let source = Source::new(tempfile.reopen()?, settings);
        let handle = source.source_handle();
        let cancellation_token = CancellationToken::new();
        let cancellation_token_ = cancellation_token.clone();
        let download_task = async move {
            source
                .download(
                    make_stream()
                        .await
                        .tap_err(|e| error!("Error creating stream: {e}"))?,
                    cancellation_token_,
                )
                .await
                .tap_err(|e| error!("Error downloading stream: {e}"))?;
            debug!("download task finished");
            Ok::<_, io::Error>(())
        };

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            debug!("tokio runtime found, spawning download task");
            handle.spawn(download_task);
        } else {
            thread::spawn(move || {
                debug!("no tokio runtime found, spawning download in separate thread");
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .tap_err(|e| error!("Error creating tokio runtime: {e}"))?;
                rt.block_on(download_task)?;
                debug!("download thread finished");
                Ok::<_, io::Error>(())
            });
        };

        Ok(Self {
            output_reader: BufReader::new(tempfile),
            handle,
            download_task_cancellation_token: cancellation_token,
        })
    }
}

impl Drop for StreamDownload {
    fn drop(&mut self) {
        self.cancel_download();
    }
}

impl Read for StreamDownload {
    #[instrument(skip_all)]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        trace!(buffer_length = buf.len(), "read requested");
        let stream_position = self.output_reader.stream_position()?;
        let requested_position = stream_position + buf.len() as u64;
        trace!(
            current_position = stream_position,
            requested_position = requested_position
        );

        if let Some(closest_set) = self.handle.downloaded().get(&stream_position) {
            trace!(
                downloaded_range = format!("{closest_set:?}"),
                "current position already downloaded"
            );
            if closest_set.end >= requested_position {
                return self.output_reader.read(buf).tap(|l| {
                    trace!(
                        read_length = format!("{l:?}"),
                        "requested position already downloaded, returning read"
                    )
                });
            } else {
                debug!("requested position not yet downloaded");
            }
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
            new_position = self.output_reader.stream_position()?,
            "reached requested position"
        );
        self.output_reader
            .read(buf)
            .tap(|l| debug!(read_length = format!("{l:?}"), "returning read"))
    }
}

impl Seek for StreamDownload {
    #[instrument(skip(self))]
    fn seek(&mut self, relative_pos: SeekFrom) -> io::Result<u64> {
        let absolute_seek_pos = match relative_pos {
            SeekFrom::Start(pos) => {
                debug!(seek_position = pos, "seeking from start");
                pos
            }
            SeekFrom::End(pos) => {
                debug!(seek_position = pos, "seeking from end");
                if let Some(length) = self.handle.content_length() {
                    (length as i64 - pos) as u64
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "cannot seek from end when content length is unknown",
                    ));
                }
            }
            SeekFrom::Current(pos) => {
                debug!(seek_position = pos, "seeking from current position");
                (self.output_reader.stream_position()? as i64 + pos) as u64
            }
        };
        debug!(absolute_seek_pos, "absolute seek position");
        if let Some(closest_set) = self.handle.downloaded().get(&absolute_seek_pos) {
            debug!(
                downloaded_range = format!("{closest_set:?}"),
                "seek position already downloaded"
            );
            return self
                .output_reader
                .seek(SeekFrom::Start(absolute_seek_pos))
                .tap(|p| debug!(position = format!("{p:?}"), "returning seek position"));
        }
        self.handle.request_position(absolute_seek_pos);
        self.handle.seek(absolute_seek_pos);
        debug!(
            requested_position = absolute_seek_pos,
            "waiting for requested position"
        );
        self.handle.wait_for_requested_position();
        debug!("reached seek position");
        self.output_reader
            .seek(SeekFrom::Start(absolute_seek_pos))
            .tap(|p| debug!(position = format!("{p:?}"), "returning seek position"))
    }
}

#[cfg(test)]
#[path = "./lib_test.rs"]
mod lib_test;
