use std::ffi::OsString;

use super::Command;

/// Helper to construct a valid `ffmpeg` command that reads from `stdin`, converts the input to the
/// provided format, and sends the output to `stdout`. This is useful for performing
/// post-processing on a media stream that needs to be converted to a different format.
#[derive(Debug)]
pub struct FfmpegConvertAudioCommand(Command);

impl From<FfmpegConvertAudioCommand> for Command {
    fn from(value: FfmpegConvertAudioCommand) -> Self {
        value.0
    }
}

impl FfmpegConvertAudioCommand {
    /// Constructs a new [`FfmpegConvertAudioCommand`].
    pub fn new<S>(format: S) -> Self
    where
        S: AsRef<str>,
    {
        // -i pipe: = pipe input from stdin
        // -qscale:a 0 = quality scale 0 (highest quality VBR)
        // -map a = select all audio streams
        // -f <format> output format
        // - = output to stdout
        Self(Command::new("ffmpeg").args([
            "-i",
            "pipe:",
            "-qscale:a",
            "0",
            "-map",
            "a",
            "-f",
            format.as_ref(),
            "-loglevel",
            "error",
            "-",
        ]))
    }

    /// Sets the path to the `ffmpeg` binary.
    #[must_use]
    pub fn ffmpeg_path<S>(mut self, path: S) -> Self
    where
        S: Into<OsString>,
    {
        self.0.program = path.into();
        self
    }
}
