use std::env::args;
use std::error::Error;
use std::num::NonZeroUsize;

use stream_download::http::HttpStream;
use stream_download::http::reqwest::Client;
use stream_download::source::{DecodeError, SourceStream};
use stream_download::storage::bounded::BoundedStorageProvider;
use stream_download::storage::memory::MemoryStorageProvider;
use stream_download::{Settings, StreamDownload};
use tracing::info;
use tracing::metadata::LevelFilter;
use tracing_subscriber::EnvFilter;

// Use the icy-metadata crate for handling Icecast metadata.
// See examples here: https://github.com/aschey/icy-metadata/tree/main/examples

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::default().add_directive(LevelFilter::INFO.into()))
        .with_line_number(true)
        .with_file(true)
        .init();

    let url = args().nth(1).unwrap_or_else(|| {
        "https://us2.internet-radio.com/proxy/mattjohnsonradio?mp=/stream".to_string()
    });

    let stream = HttpStream::<Client>::create(url.parse()?).await?;

    info!("content type={:?}", stream.content_type());
    let bitrate: u64 = stream.header("Icy-Br").unwrap().parse()?;
    info!("bitrate={bitrate}");

    // buffer 5 seconds of audio
    // bitrate (in kilobits) / bits per byte * bytes per kilobyte * 5 seconds
    let prefetch_bytes = bitrate / 8 * 1024 * 5;
    info!("prefetch bytes={prefetch_bytes}");

    let reader = match StreamDownload::from_stream(
        stream,
        // use bounded storage to keep the underlying size from growing indefinitely
        BoundedStorageProvider::new(
            MemoryStorageProvider,
            // be liberal with the buffer size, you need to make sure it holds enough space to
            // prevent any out-of-bounds reads
            NonZeroUsize::new(512 * 1024).unwrap(),
        ),
        Settings::default().prefetch_bytes(prefetch_bytes),
    )
    .await
    {
        Ok(reader) => reader,
        Err(e) => Err(e.decode_error().await)?,
    };

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
