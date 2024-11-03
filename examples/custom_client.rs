use std::error::Error;
use std::time::Instant;

use stream_download::http::{Client, HttpStream, RANGE_HEADER_KEY, format_range_header_bytes};
use stream_download::source::{DecodeError, SourceStream};
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};
use tracing::info;
use tracing::metadata::LevelFilter;
use tracing_subscriber::EnvFilter;

// If you need to perform some dynamic logic before sending each HTTP request,
// implement the `Client` trait.
// Here we show how you might pass in bearer tokens.

#[derive(Debug)]
struct BearerAuthClient(reqwest::Client);

impl Client for BearerAuthClient {
    type Url = <reqwest::Client as Client>::Url;

    type Headers = <reqwest::Client as Client>::Headers;

    type Response = <reqwest::Client as Client>::Response;

    type Error = <reqwest::Client as Client>::Error;

    fn create() -> Self {
        Self(reqwest::Client::create())
    }

    async fn get(&self, url: &Self::Url) -> Result<Self::Response, Self::Error> {
        self.0
            .get(url.clone())
            .bearer_auth(get_bearer_token())
            .send()
            .await
    }

    async fn get_range(
        &self,
        url: &Self::Url,
        start: u64,
        end: Option<u64>,
    ) -> Result<Self::Response, Self::Error> {
        self.0
            .get(url.clone())
            .bearer_auth(get_bearer_token())
            .header(RANGE_HEADER_KEY, format_range_header_bytes(start, end))
            .send()
            .await
    }
}

fn get_bearer_token() -> String {
    // In reality, you would run some custom auth logic here
    format!("fake-auth-token-{:?}", Instant::now())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::default().add_directive(LevelFilter::INFO.into()))
        .with_line_number(true)
        .with_file(true)
        .init();

    let (_stream, handle) = rodio::OutputStream::try_default()?;
    let sink = rodio::Sink::try_new(&handle)?;

    // Note: you may want to consider creating middleware using `reqwest-middleware` instead of a
    // custom client as shown here.
    // See https://docs.rs/reqwest-middleware/latest/reqwest_middleware.
    let stream = HttpStream::<BearerAuthClient>::create(
        "http://www.hyperion-records.co.uk/audiotest/14 Clementi Piano Sonata in D major, Op 25 \
         No 6 - Movement 2 Un poco andante.MP3"
            .parse()?,
    )
    .await?;

    info!("content length={:?}", stream.content_length());
    info!("content type={:?}", stream.content_type());

    let reader =
        match StreamDownload::from_stream(stream, TempStorageProvider::new(), Settings::default())
            .await
        {
            Ok(reader) => reader,
            Err(e) => Err(e.decode_error().await)?,
        };
    sink.append(rodio::Decoder::new(reader)?);

    let handle = tokio::task::spawn_blocking(move || {
        sink.sleep_until_end();
    });
    handle.await?;
    Ok(())
}
