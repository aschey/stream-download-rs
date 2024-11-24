use std::cmp::Ordering;
use std::num::NonZeroUsize;
use std::time::Duration;

use async_trait::async_trait;
use regex::Regex;
use stream_download::process::{
    CommandBuilder, FfmpegConvertAudioCommand, ProcessStreamParams, YtDlpCommand,
};
use stream_download::registry::{Input, RegistryEntry, Rule};
use stream_download::storage::adaptive::AdaptiveStorageProvider;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};
use tracing::info;
use youtube_dl::{YoutubeDl, YoutubeDlOutput};

use crate::{Reader, Result};

pub struct YtDlpResolver {
    rules: Vec<Rule>,
}

impl YtDlpResolver {
    pub fn new() -> Self {
        Self {
            rules: vec![
                Rule::prefix("ytdl://"),
                Rule::http_domain(Regex::new(r"^(www\.)?youtube\.com$").expect("invalid regex")),
                Rule::http_domain(Regex::new(r"^(www\.)?twitch\.tv$").expect("invalid regex")),
            ],
        }
    }
}

#[async_trait]
impl RegistryEntry<Result<Reader>> for YtDlpResolver {
    fn priority(&self) -> u32 {
        // Check for ytdl-specific URLs first
        1
    }

    fn rules(&self) -> &[Rule] {
        &self.rules
    }

    async fn handler(&mut self, input: Input) -> Result<Reader> {
        info!("using yt-dlp resolver");
        // These two formats are the same, but yt-dlp and ffmpeg use different terminology
        let yt_dlp_format = "m4a";
        let ffmpeg_format = "adts";

        info!("extracting video metadata - this may take a few seconds");
        let output = YoutubeDl::new(input.source.clone())
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
                let mut valid_formats: Vec<_> = formats
                    .into_iter()
                    .filter(|f| f.ext.as_deref() == Some(yt_dlp_format))
                    .collect();
                // Sort formats by quality (0 is best, 10 is worst)
                valid_formats
                    .sort_by(|a, b| a.quality.partial_cmp(&b.quality).unwrap_or(Ordering::Equal));
                // Use the best quality one
                valid_formats.pop()
            }
            YoutubeDlOutput::Playlist(playlist) => {
                info!("found playlist: {:?}", playlist.title);
                None
            }
        };
        let cmd = YtDlpCommand::new(input.source).extract_audio(true);

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
            // yt-dlp can handle format conversion, but if we want to stream it directly from
            // stdout, we have to pipe the raw output to ffmpeg ourselves.
            let builder =
                CommandBuilder::new(cmd).pipe(FfmpegConvertAudioCommand::new(ffmpeg_format));
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
        Ok(Reader::Stream(reader))
    }
}
