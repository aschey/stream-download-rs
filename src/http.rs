//! An HTTP implementation of the [SourceStream](crate::source::SourceStream) trait.
//! An implementation of the [Client](Client) trait is required to perform the actual HTTP
//! connection.
//!
//! # Example
//!
//! ```no_run
//! use std::error::Error;
//! use std::result::Result;
//!
//! use reqwest::Client;
//! use stream_download::http::HttpStream;
//! use stream_download::source::SourceStream;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn Error>> {
//!     let stream = HttpStream::new(
//!         Client::new(),
//!         "https://some-cool-url.com/some-file.mp3".parse()?,
//!     )
//!     .await?;
//!     let content_length = stream.content_length();
//!     Ok(())
//! }
//! ```

use std::error::Error;
use std::fmt::Display;
use std::io;
use std::pin::Pin;
use std::task::{self, Poll};
use std::time::Instant;

use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use tracing::{debug, instrument, warn};

use crate::source::SourceStream;

/// Wrapper trait for an HTTP client that exposes only functionality necessary for retrieving the
/// stream content. If the `reqwest` feature is enabled, this trait is implemented for
/// [reqwest::Client](https://docs.rs/reqwest/latest/reqwest/struct.Client.html).
/// This can be implemented for a custom HTTP client if desired.
#[async_trait]
pub trait Client: Send + Sync + Unpin + 'static {
    /// The HTTP URL of the remote resource.
    type Url: Display + Send + Sync + Unpin;

    /// The HTTP Response object.
    type Response: ClientResponse<Error = Self::Error>;

    /// The error type returned by HTTP requests.
    type Error: Error + Send + Sync;

    /// Creates a new instance of the client.
    fn create() -> Self;

    /// Sends an HTTP GET request to the URL.
    async fn get(&self, url: &Self::Url) -> Result<Self::Response, Self::Error>;

    /// Sends an HTTP GET request to the URL utilizing the `Range` header to request a specific part
    /// of the stream.
    async fn get_range(
        &self,
        url: &Self::Url,
        start: u64,
        end: Option<u64>,
    ) -> Result<Self::Response, Self::Error>;
}

/// A wrapper trait for an HTTP response that exposes only functionality necessary for retrieving
/// the stream content. If the `reqwest` feature is enabled,
/// this trait is implemented for
/// [reqwest::Response](https://docs.rs/reqwest/latest/reqwest/struct.Response.html).
/// This can be implemented for a custom HTTP response if desired.
pub trait ClientResponse: Send + Sync {
    /// Error type returned by the underlying response stream.
    type Error;

    /// Returns the size of the remote resource in bytes.
    /// The result should be `None` if the stream is infinite or doesn't have a known length.
    fn content_length(&self) -> Option<u64>;

    /// Checks if the response status is successful.
    fn is_success(&self) -> bool;

    /// Turns the response into an error if the response was not successful.
    fn status_error(self) -> Result<(), Self::Error>;

    /// Converts the response into a byte stream
    fn stream(self) -> Box<dyn Stream<Item = Result<Bytes, Self::Error>> + Unpin + Send + Sync>;
}

/// An HTTP implementation of the [SourceStream](crate::source::SourceStream) trait.
pub struct HttpStream<C: Client> {
    stream: Box<dyn Stream<Item = Result<Bytes, C::Error>> + Unpin + Send + Sync>,
    client: C,
    content_length: Option<u64>,
    url: C::Url,
}

impl<C: Client> HttpStream<C> {
    /// Creates a new [HttpStream](HttpStream) from a [Client](Client).
    #[instrument(skip(client, url), fields(url = url.to_string()))]
    pub async fn new(client: C, url: <Self as SourceStream>::Url) -> io::Result<Self> {
        debug!("requesting stream content");
        let request_start = Instant::now();

        let response = client
            .get(&url)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
        debug!(
            duration = format!("{:?}", request_start.elapsed()),
            "request finished"
        );
        let mut content_length = None;
        if let Some(length) = response.content_length() {
            debug!(content_length = length, "received content length");
            content_length = Some(length);
        } else {
            warn!("content length header missing");
        }
        let stream = response.stream();
        Ok(Self {
            stream: Box::new(stream),
            client,
            content_length,
            url,
        })
    }
}

impl<C: Client> Stream for HttpStream<C> {
    type Item = Result<Bytes, C::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.stream).poll_next(cx)
    }
}

#[async_trait]
impl<C: Client> SourceStream for HttpStream<C> {
    type Url = C::Url;
    type StreamError = C::Error;

    async fn create(url: Self::Url) -> io::Result<Self> {
        Self::new(C::create(), url).await
    }

    fn content_length(&self) -> Option<u64> {
        self.content_length
    }

    #[instrument(skip(self))]
    async fn seek_range(&mut self, start: u64, end: Option<u64>) -> io::Result<()> {
        if Some(start) == self.content_length {
            debug!(
                "attempting to seek where start is the length of the stream, returning empty \
                 stream"
            );
            self.stream = Box::new(futures::stream::empty());
            return Ok(());
        }
        debug!("sending HTTP range request");
        let request_start = Instant::now();
        let response = self
            .client
            .get_range(&self.url, start, end)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
        debug!(
            duration = format!("{:?}", request_start.elapsed()),
            "HTTP request finished"
        );
        if !response.is_success() {
            if let Err(e) = response.status_error() {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, e));
            } else {
                unreachable!("unsuccessful response should return an error")
            }
        }
        self.stream = Box::new(response.stream());
        debug!("done seeking");
        Ok(())
    }
}
