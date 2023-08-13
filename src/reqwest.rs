//! An implementation of the [Client](crate::http::Client) trait
//! using [reqwest](https://docs.rs/reqwest/latest/reqwest).
//! If you need to customize the client object, you can use
//! [HttpStream::new](crate::http::HttpStream::new) to supply your own reqwest client. Keep in mind
//! that reqwest recommends creating a single client and cloning it for each new connection.

use std::str::FromStr;
use std::sync::OnceLock;

use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
pub use reqwest as client;
use tap::TapFallible;
use tracing::warn;

use crate::http::{Client, ClientResponse};

impl ClientResponse for reqwest::Response {
    type Error = reqwest::Error;

    fn content_length(&self) -> Option<u64> {
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

    fn is_success(&self) -> bool {
        self.status().is_success()
    }

    fn status_error(self) -> Result<(), Self::Error> {
        self.error_for_status().map(|_| ())
    }

    fn stream(self) -> Box<dyn Stream<Item = Result<Bytes, Self::Error>> + Unpin + Send + Sync> {
        Box::new(self.bytes_stream())
    }
}

// per reqwest's docs, it's advisable to create a single client and reuse it
static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

#[async_trait]
impl Client for reqwest::Client {
    type Url = reqwest::Url;
    type Response = reqwest::Response;
    type Error = reqwest::Error;

    fn create() -> Self {
        CLIENT.get_or_init(reqwest::Client::new).clone()
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
