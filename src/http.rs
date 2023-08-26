//! An HTTP implementation of the [SourceStream] trait.
//! An implementation of the [Client] trait is required to perform the actual HTTP connection.
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
use mediatype::MediaTypeBuf;
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

    /// The type that contains the HTTP response headers.
    type Headers: ResponseHeaders;

    /// The HTTP response object.
    type Response: ClientResponse<Error = Self::Error, Headers = Self::Headers>;

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

/// Represents the content type HTTP response header
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentType {
    /// The top-level content type such as application, audio, video, etc.
    pub r#type: String,
    /// The specific subtype such as mpeg, mp4, ogg, etc.
    pub subtype: String,
}

/// A trait for getting a specific header value
pub trait ResponseHeaders: Send + Sync + Unpin {
    /// Get a specific header from the response.
    /// If the value is not present or it can't be decoded as a string, `None` is returned.
    fn header(&self, name: &str) -> Option<&str>;
}

/// A wrapper trait for an HTTP response that exposes only functionality necessary for retrieving
/// the stream content. If the `reqwest` feature is enabled,
/// this trait is implemented for
/// [reqwest::Response](https://docs.rs/reqwest/latest/reqwest/struct.Response.html).
/// This can be implemented for a custom HTTP response if desired.
pub trait ClientResponse: Send + Sync {
    /// Error type returned by the underlying response stream.
    type Error;
    /// Object containing HTTP response headers.
    type Headers: ResponseHeaders;

    /// The size of the remote resource in bytes.
    /// The result should be `None` if the stream is infinite or doesn't have a known length.
    fn content_length(&self) -> Option<u64>;

    /// The content-type response header.
    /// This should be in the standard format of `<type>/<subtype>`.
    fn content_type(&self) -> Option<&str>;

    /// Object containing HTTP response headers.
    fn headers(&self) -> Self::Headers;

    /// Checks if the response status is successful.
    fn is_success(&self) -> bool;

    /// Turns the response into an error if the response was not successful.
    fn status_error(self) -> Result<(), Self::Error>;

    /// Converts the response into a byte stream
    fn stream(self) -> Box<dyn Stream<Item = Result<Bytes, Self::Error>> + Unpin + Send + Sync>;
}

/// An HTTP implementation of the [SourceStream] trait.
pub struct HttpStream<C: Client> {
    stream: Box<dyn Stream<Item = Result<Bytes, C::Error>> + Unpin + Send + Sync>,
    client: C,
    content_length: Option<u64>,
    content_type: Option<ContentType>,
    url: C::Url,
    headers: C::Headers,
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

        let content_length = if let Some(content_length) = response.content_length() {
            debug!(content_length, "received content length");
            Some(content_length)
        } else {
            warn!("content length header missing");
            None
        };

        let content_type = if let Some(content_type) = response.content_type() {
            debug!(content_type, "received content type");
            match content_type.parse::<MediaTypeBuf>() {
                Ok(content_type) => Some(ContentType {
                    r#type: content_type.ty().to_string(),
                    subtype: content_type.subty().to_string(),
                }),
                Err(e) => {
                    warn!("error parsing content type: {e:?}");
                    None
                }
            }
        } else {
            warn!("content type header missing");
            None
        };

        let headers = response.headers();
        let stream = response.stream();
        Ok(Self {
            stream: Box::new(stream),
            client,
            content_length,
            content_type,
            headers,
            url,
        })
    }

    /// The [ContentType] of the response stream.
    pub fn content_type(&self) -> &Option<ContentType> {
        &self.content_type
    }

    /// Get a specific header from the response.
    /// If the value is not present or it can't be decoded as a string, `None` is returned.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.header(name)
    }

    /// Object containing HTTP response headers.
    pub fn headers(&self) -> &C::Headers {
        &self.headers
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
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "unknown error from HTTP range request",
                ));
            }
        }
        self.stream = Box::new(response.stream());
        debug!("done seeking");
        Ok(())
    }
}
