use std::error::Error;
use std::sync::RwLock;
use std::time::Instant;

use stream_download::http::HttpStream;
use stream_download::source::SourceStream;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload, StreamPhase};
use tracing::info;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with_line_number(true)
        .with_file(true)
        .init();

    let last_event = RwLock::new(Instant::now());
    let reader = StreamDownload::new_http(
        "http://www.hyperion-records.co.uk/audiotest/14 Clementi Piano Sonata in D major, Op 25 \
         No 6 - Movement 2 Un poco andante.MP3"
            .parse()?,
        TempStorageProvider::new(),
        Settings::default().on_progress(move |stream: &HttpStream<_>, state| {
            let now = Instant::now();
            let mut last_event = last_event.write().unwrap();
            let elapsed = now - *last_event;
            *last_event = now;
            match state.phase {
                StreamPhase::Prefetching {
                    target, chunk_size, ..
                } => {
                    info!(
                        "{:.2?} prefetch progress: {:.2}% downloaded: {:?} kb/s: {:.2}",
                        state.elapsed,
                        (state.current_position as f32 / target as f32) * 100.0,
                        state.current_chunk,
                        chunk_size as f32 / elapsed.as_nanos() as f32 * 1000.0
                    );
                }
                StreamPhase::Downloading { chunk_size, .. } => {
                    info!(
                        "{:.2?} download progress {:.2}% downloaded: {:?} kb/s: {:.2}",
                        state.elapsed,
                        (state.current_position as f32 / stream.content_length().unwrap() as f32)
                            * 100.0,
                        state.current_chunk,
                        chunk_size as f32 / elapsed.as_nanos() as f32 * 1000.0
                    );
                }
                StreamPhase::Complete => {
                    info!("{:.2?} download complete", state.elapsed);
                }
                _ => {}
            }
        }),
    )
    .await?;

    tokio::task::spawn_blocking(move || {
        let (_stream, handle) = rodio::OutputStream::try_default()?;
        let sink = rodio::Sink::try_new(&handle)?;
        sink.append(rodio::Decoder::new(reader)?);
        sink.sleep_until_end();
        Ok::<_, Box<dyn Error + Send + Sync>>(())
    })
    .await??;

    Ok(())
}
