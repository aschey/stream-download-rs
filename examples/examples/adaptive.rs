use std::env::args;
use std::error::Error;
use std::num::NonZeroUsize;

use rodio::{Decoder, DeviceSinkBuilder, Player};
use stream_download::source::DecodeError;
use stream_download::storage::adaptive::AdaptiveStorageProvider;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::default().add_directive("stream_download=trace".parse()?))
        .with_line_number(true)
        .with_file(true)
        .init();

    let Some(url) = args().nth(1) else {
        Err("Usage: cargo run --example=adaptive -- <url>")?
    };

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

    let handle = tokio::task::spawn_blocking(move || {
        let sink = DeviceSinkBuilder::open_default_sink()?;
        let player = Player::connect_new(sink.mixer());
        player.append(Decoder::new(reader)?);
        player.sleep_until_end();
        Ok::<_, Box<dyn Error + Send + Sync>>(())
    });
    handle.await??;
    Ok(())
}
