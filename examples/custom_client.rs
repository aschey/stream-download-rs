use std::error::Error;
use std::time::Duration;

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

    let client = Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .build()?;

    let stream = HttpStream::new(
        client,
        "http://www.hyperion-records.co.uk/audiotest/14 Clementi Piano Sonata in D major, Op 25 \
         No 6 - Movement 2 Un poco andante.MP3"
            .parse()?,
    )
    .await?;
    info!("Content length={:?}", stream.content_length());

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
