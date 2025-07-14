use std::env::args;
use std::error::Error;

use stream_download::process::{ProcessStreamParams, YtDlpCommand};
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};
use tracing::info;
use tracing::metadata::LevelFilter;
use tracing_subscriber::EnvFilter;
use youtube_dl::YoutubeDl;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::default().add_directive(LevelFilter::INFO.into()))
        .with_line_number(true)
        .with_file(true)
        .init();

    let url = args()
        .nth(1)
        .unwrap_or_else(|| "https://www.youtube.com/watch?v=L_XJ_s5IsQc".to_string());

    let format = "m4a";

    info!("extracting video metadata - this may take a few seconds");
    let output = YoutubeDl::new(&url)
        .format(format)
        .extract_audio(true)
        .run_async()
        .await?
        .into_single_video()
        .unwrap();
    info!("metadata extraction complete");

    let size = output.filesize.unwrap() as u64;

    let cmd = YtDlpCommand::new(url).extract_audio(true).format(format);
    let reader = StreamDownload::new_process(
        ProcessStreamParams::new(cmd)?.content_length(size),
        TempStorageProvider::new(),
        // Disable cancel_on_drop to ensure no error messages from the process are lost.
        Settings::default().cancel_on_drop(false),
    )
    .await?;
    let reader_handle = reader.handle();
    let reader = Box::new(reader);

    let handle = tokio::task::spawn_blocking(move || {
        let stream_handle = rodio::OutputStreamBuilder::open_default_stream()?;
        let sink = rodio::Sink::connect_new(stream_handle.mixer());
        sink.append(rodio::Decoder::new(reader)?);
        sink.sleep_until_end();

        Ok::<_, Box<dyn Error + Send + Sync>>(())
    });
    handle.await??;
    // Wait for the spawned subprocess to terminate gracefully
    reader_handle.wait_for_completion().await;
    Ok(())
}
