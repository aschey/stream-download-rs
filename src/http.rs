//! An HTTP implementation of the [SourceStream](crate::source::SourceStream) trait.
//! An implementation of the [Client](Client) trait is required to perform the actual HTTP connection.

use crate::source::SourceStream;
use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use std::{
    error::Error,
    fmt::Display,
    io,
    pin::Pin,
    task::{self, Poll},
    time::Instant,
};
use tracing::{debug, instrument, warn};

#[async_trait]
pub trait Client: Send + Sync + Unpin + 'static {
    type Url: Display + Send + Sync + Unpin;
    type Response: ClientResponse<Error = Self::Error>;
    type Error: Error + Send + Sync;

    fn create() -> Self;

    async fn get(&self, url: &Self::Url) -> Result<Self::Response, Self::Error>;

    async fn get_range(
        &self,
        url: &Self::Url,
        start: u64,
        end: Option<u64>,
    ) -> Result<Self::Response, Self::Error>;
}

#[async_trait]
pub trait ClientResponse: Send + Sync {
    type Error;

    async fn content_length(&self) -> Option<u64>;
    async fn is_success(&self) -> bool;
    async fn status_error(self) -> String;
    async fn stream(
        self,
    ) -> Box<dyn Stream<Item = Result<Bytes, Self::Error>> + Unpin + Send + Sync>;
}

pub struct HttpStream<C: Client> {
    stream: Box<dyn Stream<Item = Result<Bytes, C::Error>> + Unpin + Send + Sync>,
    client: C,
    content_length: Option<u64>,
    url: C::Url,
}

impl<C: Client> HttpStream<C> {
    #[instrument(skip(client, url), fields(url = url.to_string()))]
    pub async fn new(client: C, url: <Self as SourceStream>::Url) -> io::Result<Self> {
        debug!("requesting content length");
        let request_start = Instant::now();

        let response = client
            .get(&url)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
        debug!(
            duration = format!("{:?}", request_start.elapsed()),
            "content length request finished"
        );
        let mut content_length = None;
        if let Some(length) = response.content_length().await {
            debug!(content_length = length, "received content length");
            content_length = Some(length);
        } else {
            warn!("Content length header missing");
        }
        let stream = response.stream().await;
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
    type Error = C::Error;

    async fn create(url: Self::Url) -> io::Result<Self> {
        Self::new(C::create(), url).await
    }

    async fn content_length(&self) -> Option<u64> {
        self.content_length
    }

    #[instrument(skip(self))]
    async fn seek_range(&mut self, start: u64, end: Option<u64>) -> io::Result<()> {
        if Some(start) == self.content_length {
            debug!("attempting to seek where start is the length of the stream, returning empty stream");
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
        if !response.is_success().await {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                response.status_error().await,
            ));
        }
        self.stream = Box::new(response.stream().await);
        debug!("done seeking");
        Ok(())
    }
}
