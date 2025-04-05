use std::env::args;
use std::error::Error;
use std::num::NonZeroUsize;
use std::time::Duration;

use stream_download::process::{
    CommandBuilder, FfmpegConvertAudioCommand, ProcessStreamParams, YtDlpCommand,
};
use stream_download::storage::adaptive::AdaptiveStorageProvider;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};
use tracing::info;
use tracing::metadata::LevelFilter;
use tracing_subscriber::EnvFilter;
use youtube_dl::{YoutubeDl, YoutubeDlOutput};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::default().add_directive(LevelFilter::INFO.into()))
        .with_line_number(true)
        .with_file(true)
        .init();

    let Some(url) = args().nth(1) else {
        Err("Usage: cargo run --example=yt_dlp -- <url>")?
    };

    // These two formats are the same, but yt-dlp and ffmpeg use different terminology
    let yt_dlp_format = "m4a";
    let ffmpeg_format = "adts";

    info!("extracting video metadata - this may take a few seconds");
    let output = YoutubeDl::new(&url)
        .extract_audio(true)
        // --flat-playlist prevents it from enumerating all videos in the playlist, which could
        // take a long time
        .extra_arg("--flat-playlist")
        .run_async()
        .await?;
    info!("metadata extraction complete");

    let found_format = match output {
        YoutubeDlOutput::SingleVideo(video) => {
            info!("found single video: {:?}", video.title);
            let formats = video.formats.unwrap();
            let worst_score = 10.0;
            // find best format (0 is best, 10 is worst)
            formats
                .into_iter()
                .filter(|f| f.ext.as_deref() == Some(yt_dlp_format))
                .reduce(|best, format| {
                    if format.quality.unwrap_or(worst_score) < best.quality.unwrap_or(worst_score) {
                        format
                    } else {
                        best
                    }
                })
        }
        YoutubeDlOutput::Playlist(playlist) => {
            info!("found playlist: {:?}", playlist.title);
            None
        }
    };
    let cmd = YtDlpCommand::new(&url).extract_audio(true);

    let params = if let Some(format) = found_format {
        info!("source quality: {:?}", format.quality);
        info!("source is in an appropriate format, no post-processing required");
        // Prefer the explicit format ID since this insures the format used will match
        // the filesize.
        let format_id = format
            .format_id
            .unwrap_or_else(|| yt_dlp_format.to_string());
        let params = ProcessStreamParams::new(cmd.format(format_id))?;
        if let Some(size) = format.filesize {
            info!("found video size: {size}");
            params.content_length(size as u64)
        } else {
            params
        }
    } else {
        info!("source requires post-processing - converting to m4a using ffmpeg");
        // yt-dlp can handle format conversion, but if we want to stream it directly from stdout,
        // we have to pipe the raw output to ffmpeg ourselves.
        let builder = CommandBuilder::new(cmd).pipe(FfmpegConvertAudioCommand::new(ffmpeg_format));
        ProcessStreamParams::new(builder)?
    };
    let settings = Settings::default()
        // Sometimes it may take a while for ffmpeg to output a new chunk, so we can bump up the
        // retry timeout to be safe.
        .retry_timeout(Duration::from_secs(30))
        // Disable cancel_on_drop to ensure no error messages from the process are lost.
        .cancel_on_drop(false);
    let reader = StreamDownload::new_process(
        params,
        AdaptiveStorageProvider::new(
            TempStorageProvider::default(),
            // ensure we have enough buffer space to store the prefetch data
            NonZeroUsize::new((settings.get_prefetch_bytes() * 2) as usize).unwrap(),
        ),
        settings,
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
