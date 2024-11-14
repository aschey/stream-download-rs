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

    let url = "https://www.youtube.com/watch?v=L_XJ_s5IsQc";
    let format = "m4a";

    info!("extracting video metadata - this may take a few seconds");
    let output = YoutubeDl::new(url)
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
        Settings::default(),
    )
    .await?;
    let reader_handle = reader.handle();
    let reader = Box::new(reader);

    let handle = tokio::task::spawn_blocking(move || {
        let (_stream, handle) = rodio::OutputStream::try_default()?;
        let sink = rodio::Sink::try_new(&handle)?;
        sink.append(rodio::Decoder::new(reader)?);
        sink.sleep_until_end();

        Ok::<_, Box<dyn Error + Send + Sync>>(())
    });
    handle.await??;
    // Wait for the spawned subprocess to terminate gracefully
    reader_handle.wait_for_completion().await;
    Ok(())
}
