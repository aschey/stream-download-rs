use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use reqwest::Client;
use std::{
    pin::Pin,
    str::FromStr,
    task::{self, Poll},
};
use tracing::{info, warn};

use crate::source::SourceStream;

pub struct HttpStream {
    stream: Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Unpin + Send + Sync>,
    client: Client,
    content_length: Option<u64>,
    url: reqwest::Url,
}

impl Stream for HttpStream {
    type Item = Result<Bytes, reqwest::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.stream).poll_next(cx)
    }
}

#[async_trait]
impl SourceStream for HttpStream {
    type Url = reqwest::Url;
    type Error = reqwest::Error;

    async fn create(url: Self::Url) -> Self {
        let client = Client::new();
        info!("Requesting content length");
        let response = client.get(url.as_str()).send().await.unwrap();

        let mut content_length = None;
        if let Some(length) = response.headers().get(reqwest::header::CONTENT_LENGTH) {
            let length = u64::from_str(length.to_str().unwrap()).unwrap();
            info!("Got content length {length}");
            content_length = Some(length);
        } else {
            warn!("Content length header missing");
        }

        let stream = response.bytes_stream();
        Self {
            stream: Box::new(stream),
            client,
            content_length,
            url,
        }
    }

    async fn content_length(&self) -> Option<u64> {
        self.content_length
    }
    async fn seek(&mut self, pos: u64) {
        info!("Seeking");
        self.stream = Box::new(
            self.client
                .get(self.url.as_str())
                .header(
                    "Range",
                    format!(
                        "bytes={pos}-{}",
                        self.content_length
                            .map(|l| l.to_string())
                            .unwrap_or_default()
                    ),
                )
                .send()
                .await
                .unwrap()
                .bytes_stream(),
        );
        info!("Done seeking");
    }
}
