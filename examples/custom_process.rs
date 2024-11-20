use std::error::Error;

use stream_download::process::{Command, ProcessStreamParams};
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

    // This is a contrived example (there's no reason to use a process just to cat a file), but
    // it demonstrates how to use an arbitrary command as input
    let cmd = if cfg!(windows) {
        Command::new("cmd").args(["/c", "type", ".\\assets\\music.mp3"])
    } else {
        Command::new("cat").args(["./assets/music.mp3"])
    };
    let reader = StreamDownload::new_process(
        ProcessStreamParams::new(cmd)?,
        TempStorageProvider::new(),
        // Disable cancel_on_drop to ensure no error messages from the process are lost.
        Settings::default().cancel_on_drop(false),
    )
    .await?;
    let reader_handle = reader.handle();

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
