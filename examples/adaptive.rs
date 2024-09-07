use std::env::args;
use std::error::Error;
use std::num::NonZeroUsize;

use stream_download::source::DecodeError;
use stream_download::storage::adaptive::AdaptiveStorageProvider;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::default().add_directive("stream_download=trace".parse()?))
        .with_line_number(true)
        .with_file(true)
        .init();

    let Some(url) = args().nth(1) else {
        Err("Usage: cargo run --example=adaptive -- <url>")?
    };

    let (_stream, handle) = rodio::OutputStream::try_default()?;
    let sink = rodio::Sink::try_new(&handle)?;

    let settings = Settings::default();
    let reader = match StreamDownload::new_http(
        url.parse()?,
        // use adaptive storage to keep the underlying size bounded when the stream has no content
        // length
        AdaptiveStorageProvider::new(
            TempStorageProvider::default(),
            // ensure we have enough buffer space to store the prefetch data
            NonZeroUsize::new((settings.get_prefetch_bytes() * 2) as usize).unwrap(),
        ),
        settings,
    )
    .await
    {
        Ok(reader) => reader,
        Err(e) => return Err(e.decode_error().await)?,
    };
    sink.append(rodio::Decoder::new(reader)?);

    let handle = tokio::task::spawn_blocking(move || {
        sink.sleep_until_end();
    });
    handle.await?;
    Ok(())
}
