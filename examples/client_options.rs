use std::error::Error;
use std::time::Duration;

use axum::http::{HeaderMap, HeaderValue};
use stream_download::http::reqwest::Client;
use stream_download::http::HttpStream;
use stream_download::source::SourceStream;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};
use tracing::info;
use tracing::metadata::LevelFilter;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::default().add_directive(LevelFilter::INFO.into()))
        .with_line_number(true)
        .with_file(true)
        .init();

    let (_stream, handle) = rodio::OutputStream::try_default()?;
    let sink = rodio::Sink::try_new(&handle)?;

    // If you need to add some custom options to your HTTP client,
    // construct it manually and pass it into `HttpStream::new`.
    let mut headers = HeaderMap::new();
    // For example, you may need to add some authentication headers with every request.
    // If you need to support a more complex authentication scheme, see the `custom_client` example.
    let mut header: HeaderValue = "someApiKey".parse()?;
    header.set_sensitive(true);
    headers.insert("X-Api-Key", header);

    // Keep in mind that reqwest recommends reusing a single client instance
    // so you may want to store this somewhere
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .default_headers(headers)
        .build()?;

    let stream = HttpStream::new(
        client,
        "http://www.hyperion-records.co.uk/audiotest/14 Clementi Piano Sonata in D major, Op 25 \
         No 6 - Movement 2 Un poco andante.MP3"
            .parse()?,
    )
    .await?;

    info!("content length={:?}", stream.content_length());
    info!("content type={:?}", stream.content_type());

    let reader =
        StreamDownload::from_stream(stream, TempStorageProvider::new(), Settings::default())
            .await?;
    sink.append(rodio::Decoder::new(reader)?);

    let handle = tokio::task::spawn_blocking(move || {
        sink.sleep_until_end();
    });
    handle.await?;
    Ok(())
}
