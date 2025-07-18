use std::error::Error;
use std::io::IsTerminal;

use stream_download::async_read::AsyncReadStreamParams;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};
use tracing::metadata::LevelFilter;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::default().add_directive(LevelFilter::INFO.into()))
        .with_line_number(true)
        .with_file(true)
        .init();

    if std::io::stdin().is_terminal() {
        if cfg!(windows) {
            Err(
                "Pipe in an input stream. Ex: Get-Content ./assets/music.mp3 -AsByteStream | \
                 cargo run --example=stdin --features=async-read",
            )?;
        } else {
            Err(
                "Pipe in an input stream. Ex: cat ./assets/music.mp3 | cargo run --example=stdin \
                 --features=async-read",
            )?;
        }
    }

    let reader = StreamDownload::new_async_read(
        AsyncReadStreamParams::new(tokio::io::stdin()),
        TempStorageProvider::new(),
        Settings::default(),
    )
    .await?;

    let handle = tokio::task::spawn_blocking(move || {
        let stream_handle = rodio::OutputStreamBuilder::open_default_stream()?;
        let sink = rodio::Sink::connect_new(stream_handle.mixer());
        sink.append(rodio::Decoder::new(reader)?);
        sink.sleep_until_end();

        Ok::<_, Box<dyn Error + Send + Sync>>(())
    });
    handle.await??;
    Ok(())
}
