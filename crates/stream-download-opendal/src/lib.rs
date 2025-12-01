#![deny(missing_docs)]
#![forbid(clippy::unwrap_used)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

use std::fmt::Debug;
use std::future::Future;
use std::io::{self};
use std::num::NonZeroUsize;
use std::task::Poll;

use bytes::{Bytes, BytesMut};
use futures_util::{Stream, ready};
use opendal::{FuturesAsyncReader, Operator, Reader};
use pin_project_lite::pin_project;
use stream_download::source::{DecodeError, SourceStream};
use stream_download::storage::{ContentLength, StorageProvider};
use stream_download::{Settings, StreamDownload, StreamInitializationError};
use tokio_util::compat::{Compat, FuturesAsyncReadCompatExt};
use tokio_util::io::poll_read_buf;
use tracing::instrument;

/// Extension trait for adding `OpenDAL` support to a [`StreamDownload`] instance.
pub trait StreamDownloadExt<P>
where
    Self: Sized,
{
    /// Creates a new [`StreamDownload`] that uses an `OpenDAL` resource.
    /// See the [`opendal`] documentation for more details.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::error::Error;
    /// use std::io::{self, Read};
    /// use std::result::Result;
    ///
    /// use opendal::{Operator, services};
    /// use stream_download::storage::temp::TempStorageProvider;
    /// use stream_download::{Settings, StreamDownload};
    /// use stream_download_opendal::{OpendalStreamParams, StreamDownloadExt};
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
    ///     let mut reader = StreamDownload::new_opendal(
    ///         OpendalStreamParams::new(operator, "some-object-key"),
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
    fn new_opendal(
        params: OpendalStreamParams,
        storage_provider: P,
        settings: Settings<OpendalStream>,
    ) -> impl Future<Output = Result<Self, StreamInitializationError<OpendalStream>>> + Send;
}

impl<P: StorageProvider> StreamDownloadExt<P> for StreamDownload<P> {
    async fn new_opendal(
        params: OpendalStreamParams,
        storage_provider: P,
        settings: Settings<OpendalStream>,
    ) -> Result<Self, StreamInitializationError<OpendalStream>> {
        Self::new(params, storage_provider, settings).await
    }
}

/// Parameters for creating an `OpenDAL` stream.
#[derive(Debug, Clone)]
pub struct OpendalStreamParams {
    operator: Operator,
    path: String,
    chunk_size: usize,
}

impl OpendalStreamParams {
    /// Creates a new [`OpendalStreamParams`] instance.
    pub fn new<S>(operator: Operator, path: S) -> Self
    where
        S: Into<String>,
    {
        Self {
            operator,
            path: path.into(),
            chunk_size: 4096,
        }
    }

    /// Sets the chunk size for the [`OpendalStream`].
    /// The default value is 4096.
    #[must_use]
    pub fn chunk_size(mut self, chunk_size: NonZeroUsize) -> Self {
        self.chunk_size = chunk_size.get();
        self
    }
}

pin_project! {
    /// An `OpenDAL` implementation of the [`SourceStream`] trait
    pub struct OpendalStream {
        #[pin]
        async_reader: Compat<FuturesAsyncReader>,
        reader: Reader,
        buf: BytesMut,
        capacity: usize,
        content_length: ContentLength,
        content_type: Option<String>,
    }
}

// Can't use educe here because of https://github.com/taiki-e/pin-project-lite/issues/3
impl Debug for OpendalStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpendalStream")
            .field("async_reader", &"<async_reader>")
            .field("reader", &"<reader>")
            .field("buf", &self.buf)
            .field("capacity", &self.capacity)
            .field("content_length", &self.content_length)
            .field("content_type", &self.content_type)
            .finish()
    }
}

/// Error returned from `OpenDAL`
#[derive(thiserror::Error, Debug)]
#[error("{0}")]
pub struct Error(#[from] opendal::Error);

impl DecodeError for Error {}

impl OpendalStream {
    /// Creates a new [`OpendalStream`].
    #[instrument]
    pub async fn new(params: OpendalStreamParams) -> Result<Self, Error> {
        let stat = params.operator.stat(&params.path).await?;
        let content_type = stat.content_type().map(ToString::to_string);
        let reader = params.operator.reader(&params.path).await?;
        let async_reader = reader.clone().into_futures_async_read(..).await?.compat();

        let content_length = stat.content_length();
        let content_length = if content_length > 0 {
            ContentLength::Static(content_length)
        } else {
            ContentLength::Unknown
        };

        Ok(Self {
            async_reader,
            reader,
            buf: BytesMut::with_capacity(params.chunk_size),
            capacity: params.chunk_size,
            content_length,
            content_type,
        })
    }

    /// Returns the content type of the stream, if it is known.
    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }
}

impl SourceStream for OpendalStream {
    type Params = OpendalStreamParams;

    type StreamCreationError = Error;

    async fn create(params: Self::Params) -> Result<Self, Self::StreamCreationError> {
        Self::new(params).await
    }

    fn content_length(&self) -> ContentLength {
        self.content_length
    }

    async fn seek_range(&mut self, start: u64, end: Option<u64>) -> io::Result<()> {
        let reader = self.reader.clone();
        let async_reader = match end {
            Some(end) => reader.into_futures_async_read(start..end).await,
            None => reader.into_futures_async_read(start..).await,
        };

        self.async_reader = async_reader
            .map_err(Into::into)
            .wrap_err("error creating async reader")?
            .compat();
        Ok(())
    }

    async fn reconnect(&mut self, current_position: u64) -> io::Result<()> {
        self.seek_range(current_position, None).await
    }

    fn supports_seek(&self) -> bool {
        true
    }
}

impl Stream for OpendalStream {
    type Item = io::Result<Bytes>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let mut this = self.as_mut().project();

        if this.buf.capacity() == 0 {
            this.buf.reserve(*this.capacity);
        }

        match ready!(poll_read_buf(this.async_reader, cx, &mut this.buf)) {
            Err(err) => Poll::Ready(Some(Err(err))),
            Ok(0) => Poll::Ready(None),
            Ok(_) => {
                let chunk = this.buf.split();
                Poll::Ready(Some(Ok(chunk.freeze())))
            }
        }
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
