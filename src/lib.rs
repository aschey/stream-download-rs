use source::{Settings, Source, SourceHandle, SourceStream};
use std::{
    future::{self, Future},
    io::{self, BufReader, Read, Seek, SeekFrom},
    thread,
};
use tap::{Tap, TapFallible};
use tempfile::NamedTempFile;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, instrument, trace};

#[cfg(feature = "http")]
pub mod http;
pub mod source;

#[derive(Debug)]
pub struct StreamDownload {
    output_reader: BufReader<NamedTempFile>,
    handle: SourceHandle,
    download_task_cancellation_token: CancellationToken,
}

impl StreamDownload {
    #[cfg(feature = "http")]
    pub fn new_http(url: reqwest::Url, settings: Settings) -> io::Result<Self> {
        Self::from_make_stream(
            move || http::HttpStream::new(reqwest::Client::new(), url),
            settings,
        )
    }

    pub fn new<S: SourceStream>(url: S::Url, settings: Settings) -> io::Result<Self> {
        Self::from_make_stream(move || S::create(url), settings)
    }

    pub fn from_stream<S: SourceStream>(stream: S, settings: Settings) -> Result<Self, io::Error> {
        Self::from_make_stream(move || future::ready(Ok(stream)), settings)
    }

    fn from_make_stream<S, F, Fut>(make_stream: F, settings: Settings) -> Result<Self, io::Error>
    where
        S: SourceStream,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = io::Result<S>> + Send,
    {
        let tempfile = tempfile::Builder::new().tempfile()?;
        let source = Source::new(tempfile.reopen()?, settings);
        let handle = source.source_handle();
        let cancellation_token = CancellationToken::new();
        let cancellation_token_ = cancellation_token.clone();
        let download_task = async move {
            source
                .download(
                    make_stream()
                        .await
                        .tap_err(|e| error!("Error creating stream: {e}"))?,
                    cancellation_token_,
                )
                .await
                .tap_err(|e| error!("Error downloading stream: {e}"))?;
            debug!("download task finished");
            Ok::<_, io::Error>(())
        };

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            debug!("tokio runtime found, spawning download task");
            handle.spawn(download_task);
        } else {
            thread::spawn(move || {
                debug!("no tokio runtime found, spawning download in separate thread");
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .tap_err(|e| error!("Error creating tokio runtime: {e}"))?;
                rt.block_on(download_task)?;
                debug!("download thread finished");
                Ok::<_, io::Error>(())
            });
        };

        Ok(Self {
            output_reader: BufReader::new(tempfile),
            handle,
            download_task_cancellation_token: cancellation_token,
        })
    }

    pub fn cancel_download(&self) {
        self.download_task_cancellation_token.cancel();
    }
}

impl Drop for StreamDownload {
    fn drop(&mut self) {
        self.cancel_download();
    }
}

impl Read for StreamDownload {
    #[instrument(skip_all)]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        trace!(buffer_length = buf.len(), "read requested");
        let stream_position = self.output_reader.stream_position()?;
        let requested_position = stream_position + buf.len() as u64;
        trace!(
            current_position = stream_position,
            requested_position = requested_position
        );

        if let Some(closest_set) = self.handle.downloaded().get(&stream_position) {
            trace!(
                downloaded_range = format!("{closest_set:?}"),
                "current position already downloaded"
            );
            if closest_set.end >= requested_position {
                return self.output_reader.read(buf).tap(|l| {
                    trace!(
                        read_length = format!("{l:?}"),
                        "requested position already downloaded, returning read"
                    )
                });
            } else {
                debug!("requested position not yet downloaded");
            }
        } else {
            debug!("stream position not yet downloaded");
        }

        self.handle.request_position(requested_position);
        debug!(
            requested_position = requested_position,
            "waiting for requested position"
        );
        self.handle.wait_for_requested_position();
        debug!(
            current_position = stream_position,
            requested_position = requested_position,
            new_position = self.output_reader.stream_position()?,
            "reached requested position"
        );
        self.output_reader
            .read(buf)
            .tap(|l| debug!(read_length = format!("{l:?}"), "returning read"))
    }
}

impl Seek for StreamDownload {
    #[instrument(skip(self))]
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let seek_pos = match pos {
            SeekFrom::Start(pos) => {
                debug!(seek_position = pos, "seeking from start");
                pos
            }
            SeekFrom::End(pos) => {
                debug!(seek_position = pos, "seeking from end");
                if let Some(length) = self.handle.content_length() {
                    (length as i64 - 1 + pos) as u64
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "cannot seek from end when content length is unknown",
                    ));
                }
            }
            SeekFrom::Current(pos) => {
                debug!(seek_position = pos, "seeking from current position");
                (self.output_reader.stream_position()? as i64 + pos) as u64
            }
        };
        if let Some(closest_set) = self.handle.downloaded().get(&seek_pos) {
            debug!(
                downloaded_range = format!("{closest_set:?}"),
                "seek position already downloaded"
            );
            return self
                .output_reader
                .seek(pos)
                .tap(|p| debug!(position = format!("{p:?}"), "returning seek position"));
        }
        self.handle.request_position(seek_pos);
        self.handle.seek(seek_pos);
        debug!(
            requested_position = seek_pos,
            "waiting for requested position"
        );
        self.handle.wait_for_requested_position();
        debug!("reached seek position");
        self.output_reader
            .seek(pos)
            .tap(|p| debug!(position = format!("{p:?}"), "returning seek position"))
    }
}

#[cfg(test)]
#[path = "./lib_test.rs"]
mod lib_test;
