//! A [`SourceStream`] adapter for [OpenDAL](https://docs.rs/opendal/latest/opendal).
//! `OpenDAL` is a data access layer that supports data retrieval from a variety of storage
//! services. The list of supported services is [documented here](https://docs.rs/opendal/latest/opendal/services/index.html).
//!
//! # Example using S3
//!
//! ```no_run
//! use std::error::Error;
//!
//! use opendal::{services, Operator};
//! use stream_download::open_dal::{OpenDalStream, OpenDalStreamParams};
//! use stream_download::storage::temp::TempStorageProvider;
//! use stream_download::{Settings, StreamDownload};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn Error>> {
//!     let mut builder = services::S3::default()
//!         .region("us-east-1")
//!         .access_key_id("test")
//!         .secret_access_key("test")
//!         .bucket("my-bucket");
//!
//!     let operator = Operator::new(builder)?.finish();
//!     let stream =
//!         OpenDalStream::new(OpenDalStreamParams::new(operator, "some-object-key")).await?;
//!
//!     Ok(())
//! }
//! ```

use std::fmt::Debug;
use std::io::{self};
use std::num::NonZeroUsize;
use std::task::Poll;

use bytes::{Bytes, BytesMut};
use futures::{ready, Stream};
use opendal::{FuturesAsyncReader, Metakey, Operator, Reader};
use pin_project_lite::pin_project;
use tokio_util::compat::{Compat, FuturesAsyncReadCompatExt};
use tokio_util::io::poll_read_buf;
use tracing::instrument;

use crate::source::SourceStream;
use crate::WrapIoResult;

/// Parameters for creating an `OpenDAL` stream.
#[derive(Debug, Clone)]
pub struct OpenDalStreamParams {
    operator: Operator,
    path: String,
    chunk_size: usize,
}

impl OpenDalStreamParams {
    /// Creates a new [`OpenDalStreamParams`] instance.
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

    /// Sets the chunk size for the [`OpenDalStream`].
    /// The default value is 4096.
    pub fn chunk_size(mut self, chunk_size: NonZeroUsize) -> Self {
        self.chunk_size = chunk_size.get();
        self
    }
}

pin_project! {
    /// An `OpenDAL` implementation of the [`SourceStream`] trait
    pub struct OpenDalStream {
        #[pin]
        async_reader: Compat<FuturesAsyncReader>,
        reader: Reader,
        buf: BytesMut,
        capacity: usize,
        content_length: Option<u64>,
        content_type: Option<String>,
    }
}

// Can't use educe here because of https://github.com/taiki-e/pin-project-lite/issues/3
impl Debug for OpenDalStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenDalStream")
            .field("async_reader", &"<async_reader>")
            .field("reader", &"<reader>")
            .field("buf", &self.buf)
            .field("capacity", &self.capacity)
            .field("content_length", &self.content_length)
            .field("content_type", &self.content_type)
            .finish()
    }
}

impl OpenDalStream {
    /// Creates a new [`OpenDalStream`].
    #[instrument]
    pub async fn new(params: OpenDalStreamParams) -> io::Result<Self> {
        let stat = params
            .operator
            .stat(&params.path)
            .await
            .map_err(|e| e.into())
            .wrap_err("error fetching metadata")?;

        let content_length = if stat.metakey().contains(Metakey::ContentLength) {
            // content_length() will panic if called when the ContentLength Metakey is not present
            Some(stat.content_length())
        } else {
            None
        };

        let content_type = stat.content_type().map(|t| t.to_string());

        let reader = params
            .operator
            .reader(&params.path)
            .await
            .map_err(|e| e.into())
            .wrap_err("error creating reader")?;

        let async_reader = reader
            .clone()
            .into_futures_async_read(..)
            .await
            .map_err(|e| e.into())
            .wrap_err("error creating async reader")?
            .compat();

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

impl SourceStream for OpenDalStream {
    type Params = OpenDalStreamParams;

    type StreamError = io::Error;

    async fn create(params: Self::Params) -> io::Result<Self> {
        Self::new(params).await
    }

    fn content_length(&self) -> Option<u64> {
        self.content_length
    }

    async fn seek_range(&mut self, start: u64, end: Option<u64>) -> io::Result<()> {
        let reader = self.reader.clone();
        let async_reader = match end {
            Some(end) => reader.into_futures_async_read(start..end).await,
            None => reader.into_futures_async_read(start..).await,
        };

        self.async_reader = async_reader
            .map_err(|e| e.into())
            .wrap_err("error creating async reader")?
            .compat();
        Ok(())
    }
}

impl Stream for OpenDalStream {
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
