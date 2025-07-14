use std::env::args;
use std::error::Error;

use stream_download::source::DecodeError;
use stream_download::storage::memory::MemoryStorageProvider;
use stream_download::{Settings, StreamDownload};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::default().add_directive("stream_download=trace".parse()?))
        .with_line_number(true)
        .with_file(true)
        .init();

    let url = args().nth(1).unwrap_or_else(|| {
        "http://www.hyperion-records.co.uk/audiotest/14 Clementi Piano Sonata in D major, Op 25 No \
         6 - Movement 2 Un poco andante.MP3"
            .to_string()
    });

    let reader =
        match StreamDownload::new_http(url.parse()?, MemoryStorageProvider, Settings::default())
            .await
        {
            Ok(reader) => reader,
            Err(e) => return Err(e.decode_error().await)?,
        };

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
