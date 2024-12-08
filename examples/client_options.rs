use std::env::args;
use std::error::Error;
use std::time::Duration;

use axum::http::{HeaderMap, HeaderValue};
use stream_download::http::HttpStream;
use stream_download::http::reqwest::Client;
use stream_download::source::{DecodeError, SourceStream};
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};
use tracing::info;
use tracing::metadata::LevelFilter;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::default().add_directive(LevelFilter::INFO.into()))
        .with_line_number(true)
        .with_file(true)
        .init();

    let url = args().nth(1).unwrap_or_else(|| {
        "http://www.hyperion-records.co.uk/audiotest/14 Clementi Piano Sonata in D major, Op 25 No \
         6 - Movement 2 Un poco andante.MP3"
            .to_string()
    });

    // If you need to add some custom options to your HTTP client,
    // construct it manually and pass it into `HttpStream::new`.
    let mut headers = HeaderMap::new();
    // For example, you may need to add some authentication headers with every request.
    // If you need to support a more complex authentication scheme, see the `custom_client` example
    // or consider using `reqwest_middleware` (https://docs.rs/reqwest-middleware/latest/reqwest_middleware).
    let mut header: HeaderValue = "someApiKey".parse()?;
    header.set_sensitive(true);
    headers.insert("X-Api-Key", header);

    // Keep in mind that reqwest recommends reusing a single client instance
    // so you may want to store this somewhere
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .default_headers(headers)
        .build()?;

    let stream = match HttpStream::new(client, url.parse()?).await {
        Ok(stream) => stream,
        Err(e) => return Err(e.decode_error().await)?,
    };

    info!("content length={:?}", stream.content_length());
    info!("content type={:?}", stream.content_type());

    let reader =
        StreamDownload::from_stream(stream, TempStorageProvider::new(), Settings::default())
            .await?;

    let handle = tokio::task::spawn_blocking(move || {
        let (_stream, handle) = rodio::OutputStream::try_default()?;
        let sink = rodio::Sink::try_new(&handle)?;
        sink.append(rodio::Decoder::new(reader)?);
        sink.sleep_until_end();
        Ok::<_, Box<dyn Error + Send + Sync>>(())
    });
    handle.await??;
    Ok(())
}
