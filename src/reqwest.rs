//! An implementation of the [Client](crate::http::Client) trait using [reqwest](https://docs.rs/reqwest/latest/reqwest).

use crate::http::{Client, ClientResponse};
use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use std::str::FromStr;
use tap::TapFallible;
use tracing::warn;

#[async_trait]
impl ClientResponse for reqwest::Response {
    type Error = reqwest::Error;

    async fn content_length(&self) -> Option<u64> {
        if let Some(length) = self.headers().get(reqwest::header::CONTENT_LENGTH) {
            let content_length = length
                .to_str()
                .tap_err(|e| warn!("error getting length response: {e:?}"))
                .ok();

            content_length.and_then(|l| {
                u64::from_str(l)
                    .tap_err(|e| warn!("invalid content length value: {e:?}"))
                    .ok()
            })
        } else {
            None
        }
    }

    async fn is_success(&self) -> bool {
        self.status().is_success()
    }

    async fn status_error(self) -> String {
        match self.error_for_status() {
            Ok(_) => String::default(),
            Err(e) => e.to_string(),
        }
    }

    async fn stream(
        self,
    ) -> Box<dyn Stream<Item = Result<Bytes, Self::Error>> + Unpin + Send + Sync> {
        Box::new(self.bytes_stream())
    }
}

#[async_trait]
impl Client for reqwest::Client {
    type Url = reqwest::Url;
    type Response = reqwest::Response;
    type Error = reqwest::Error;

    fn create() -> Self {
        reqwest::Client::new()
    }

    async fn get(&self, url: &Self::Url) -> Result<Self::Response, Self::Error> {
        self.get(url.clone()).send().await
    }

    async fn get_range(
        &self,
        url: &Self::Url,
        start: u64,
        end: Option<u64>,
    ) -> Result<Self::Response, Self::Error> {
        self.get(url.clone())
            .header(
                "Range",
                format!(
                    "bytes={start}-{}",
                    end.map(|e| e.to_string()).unwrap_or_default()
                ),
            )
            .send()
            .await
    }
}
