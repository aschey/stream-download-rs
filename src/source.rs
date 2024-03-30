//! Provides the [`SourceStream`] trait which abstracts over the transport used to
//! stream remote content.
use std::error::Error;
use std::io::{self, SeekFrom};
use std::ops::Range;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use bytes::Bytes;
use futures::{Future, Stream, StreamExt};
use parking_lot::{Condvar, Mutex, RwLock};
use rangemap::RangeSet;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, instrument, trace};

use crate::storage::StorageWriter;
use crate::Settings;

/// Represents a remote resource that can be streamed over the network. Streaming
/// over http is implemented via the [HttpStream](crate::http::HttpStream)
/// implementation if the `http` feature is enabled.
///
/// The implementation must also implement the
/// [Stream](https://docs.rs/futures/latest/futures/stream/trait.Stream.html) trait.
pub trait SourceStream:
    Stream<Item = Result<Bytes, Self::StreamError>> + Unpin + Send + Sync + Sized + 'static
{
    /// URL of the remote resource.
    type Url: Send;

    /// Error type thrown by the underlying stream.
    type StreamError: Error + Send;

    /// Creates an instance of the stream.
    fn create(url: Self::Url) -> impl Future<Output = io::Result<Self>> + Send;

    /// Returns the size of the remote resource in bytes. The result should be `None`
    /// if the stream is infinite or doesn't have a known length.
    fn content_length(&self) -> Option<u64>;

    /// Seeks to a specific position in the stream. This method is only called if the
    /// requested range has not been downloaded, so this method should jump to the
    /// requested position in the stream as quickly as possible.
    fn seek_range(
        &mut self,
        start: u64,
        end: Option<u64>,
    ) -> impl Future<Output = io::Result<()>> + Send;
}

#[derive(Debug, Clone)]
pub(crate) struct SourceHandle {
    downloaded: Downloaded,
    requested_position: RequestedPosition,
    position_reached: PositionReached,
    content_length: Option<u64>,
    seek_tx: mpsc::Sender<u64>,
}

impl SourceHandle {
    pub fn get_downloaded_at_position(&self, position: u64) -> Option<Range<u64>> {
        self.downloaded.get(position)
    }

    pub fn request_position(&self, position: u64) {
        self.requested_position.set(position);
    }

    pub fn wait_for_requested_position(&self) {
        self.position_reached.wait_for_position_reached()
    }

    pub fn seek(&self, position: u64) {
        self.seek_tx.try_send(position).ok();
    }

    pub fn content_length(&self) -> Option<u64> {
        self.content_length
    }
}

#[derive(Debug, Clone)]
struct RequestedPosition(Arc<AtomicI64>);

impl Default for RequestedPosition {
    fn default() -> Self {
        Self(Arc::new(AtomicI64::new(-1)))
    }
}

// relaxed ordering as we are not using the atomic to
// lock something or to synchronize threads.
impl RequestedPosition {
    fn clear(&self) {
        self.0.store(-1, Ordering::Relaxed);
    }

    fn get(&self) -> Option<u64> {
        let val = self.0.load(Ordering::Relaxed);
        if val == -1 { None } else { Some(val as u64) }
    }

    fn set(&self, position: u64) {
        self.0.store(position as i64, Ordering::Relaxed);
    }
}

#[derive(Default, Clone, Debug)]
struct PositionReached(Arc<(Mutex<Waiter>, Condvar)>);

impl PositionReached {
    fn notify_position_reached(&self) {
        let (mutex, cvar) = self.0.as_ref();
        mutex.lock().position_reached = true;
        cvar.notify_all();
    }

    fn notify_stream_done(&self) {
        let (mutex, cvar) = self.0.as_ref();
        mutex.lock().stream_done = true;
        cvar.notify_all();
    }

    fn wait_for_position_reached(&self) {
        let (mutex, cvar) = self.0.as_ref();
        let mut waiter = mutex.lock();
        if waiter.stream_done {
            return;
        }

        #[cfg(debug_assertions)]
        let wait_start = std::time::Instant::now();

        cvar.wait_while(&mut waiter, |waiter| {
            !waiter.stream_done && !waiter.position_reached
        });

        #[cfg(debug_assertions)]
        debug!(
            elapsed = format!("{:?}", wait_start.elapsed()),
            "position reached"
        );
        if !waiter.stream_done {
            waiter.position_reached = false;
        }
    }
}

#[derive(Debug, Clone, Default)]
struct Downloaded(Arc<RwLock<RangeSet<u64>>>);

impl Downloaded {
    fn add(&self, range: Range<u64>) {
        if range.end > range.start {
            self.0.write().insert(range)
        }
    }

    fn get(&self, position: u64) -> Option<Range<u64>> {
        self.0.read().get(&position).cloned()
    }

    fn next_gap(&self, range: &Range<u64>) -> Option<Range<u64>> {
        self.0.read().gaps(range).next()
    }
}

#[derive(Default, Debug)]
struct Waiter {
    position_reached: bool,
    stream_done: bool,
}

#[derive(PartialEq, Eq)]
enum DownloadStatus {
    Continue,
    Complete,
}

pub(crate) struct Source<W: StorageWriter> {
    writer: W,
    downloaded: Downloaded,
    requested_position: RequestedPosition,
    position_reached: PositionReached,
    content_length: Option<u64>,
    seek_tx: mpsc::Sender<u64>,
    seek_rx: mpsc::Receiver<u64>,
    settings: Settings,
    prefetch_complete: bool,
}

impl<H: StorageWriter> Source<H> {
    pub(crate) fn new(writer: H, content_length: Option<u64>, settings: Settings) -> Self {
        let (seek_tx, seek_rx) = mpsc::channel(32);
        Self {
            writer,
            downloaded: Default::default(),
            requested_position: RequestedPosition::default(),
            position_reached: Default::default(),
            seek_tx,
            seek_rx,
            content_length,
            prefetch_complete: settings.prefetch_bytes == 0,
            settings,
        }
    }

    #[instrument(skip_all)]
    pub(crate) async fn download<S: SourceStream>(
        mut self,
        mut stream: S,
        cancellation_token: CancellationToken,
    ) -> io::Result<()> {
        debug!("starting file download");

        #[cfg(debug_assertions)]
        let download_start = std::time::Instant::now();

        loop {
            tokio::select! {
                position = self.seek_rx.recv() => {
                    let position = position.expect("seek_tx dropped");
                    if self.should_seek(position)? {
                        debug!("seek position not yet downloaded");

                        if !self.prefetch_complete {
                            debug!("seeking during prefetch, ending prefetch early");
                            self.downloaded.add(0.. self.writer.stream_position()?);
                            self.prefetch_complete = true;
                        }
                        self.seek(&mut stream, position, None).await?;
                    }
                    continue
                },
                bytes = stream.next() => {
                    if self.handle_bytes(&mut stream, bytes).await? == DownloadStatus::Complete {
                        #[cfg(debug_assertions)]
                        debug!(
                            download_duration = format!("{:?}", download_start.elapsed()),
                            "stream finished downloading"
                        );
                        break;
                    }
                }
                _ = cancellation_token.cancelled() => {
                    debug!("received cancellation request, stopping download task");
                    self.writer.flush()?;
                    self.signal_download_complete();
                    return Ok(())
                }
            };
        }
        Ok(())
    }

    async fn handle_prefetch<S: SourceStream>(
        &mut self,
        stream: &mut S,
        bytes: Option<Bytes>,
    ) -> io::Result<DownloadStatus> {
        let Some(bytes) = bytes else {
            self.prefetch_complete = true;
            debug!("file shorter than prefetch length, download finished");
            self.writer.flush()?;
            let position = self.writer.stream_position()?;
            self.downloaded.add(0..position);

            return self.finish_or_find_next_gap(stream).await;
        };

        self.writer.write_all(&bytes)?;
        self.writer.flush()?;
        let stream_position = self.writer.stream_position()?;

        #[cfg(debug_assertions)]
        trace!(
            stream_position = stream_position,
            prefetch_target = self.settings.prefetch_bytes,
            progress = format!(
                "{:.2}%",
                (stream_position as f32 / self.settings.prefetch_bytes as f32) * 100.0
            ),
            "prefetch"
        );

        if stream_position >= self.settings.prefetch_bytes {
            self.downloaded.add(0..stream_position);
            self.prefetch_complete = true;
        }

        Ok(DownloadStatus::Continue)
    }

    async fn finish_or_find_next_gap<S: SourceStream>(
        &mut self,
        stream: &mut S,
    ) -> io::Result<DownloadStatus> {
        if let Some(content_length) = self.content_length {
            let gap = self.get_download_gap(content_length);
            if let Some(gap) = gap {
                debug!(
                    missing = format!("{gap:?}"),
                    "downloading missing stream chunk"
                );
                self.seek(stream, gap.start, Some(gap.end)).await?;
                return Ok(DownloadStatus::Continue);
            }
        }
        self.writer.flush()?;
        self.signal_download_complete();
        Ok(DownloadStatus::Complete)
    }

    async fn handle_bytes<S: SourceStream>(
        &mut self,
        stream: &mut S,
        bytes: Option<Result<Bytes, S::StreamError>>,
    ) -> io::Result<DownloadStatus> {
        let bytes = match bytes.transpose() {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Error fetching chunk from stream: {e:?}");
                return Ok(DownloadStatus::Continue);
            }
        };

        if !self.prefetch_complete {
            return self.handle_prefetch(stream, bytes).await;
        }

        let Some(bytes) = bytes else {
            return self.finish_or_find_next_gap(stream).await;
        };

        let position = self.writer.stream_position()?;
        self.writer.write_all(&bytes)?;
        self.writer.flush()?;
        let new_position = self.writer.stream_position()?;

        trace!(
            previous_position = position,
            new_position,
            chunk_size = bytes.len(),
            "received response chunk",
        );

        self.downloaded.add(position..new_position);
        if let Some(requested) = self.requested_position.get() {
            debug!(
                requested_position = requested,
                current_position = new_position,
                "received requested position"
            );

            if new_position >= requested {
                self.requested_position.clear();
                self.position_reached.notify_position_reached()
            }
        }
        Ok(DownloadStatus::Continue)
    }

    fn should_seek(&mut self, position: u64) -> io::Result<bool> {
        Ok(if let Some(range) = self.downloaded.get(position) {
            !range.contains(&self.writer.stream_position()?)
        } else {
            true
        })
    }

    async fn seek<S: SourceStream>(
        &mut self,
        stream: &mut S,
        start: u64,
        end: Option<u64>,
    ) -> io::Result<()> {
        stream.seek_range(start, end).await?;
        self.writer.seek(SeekFrom::Start(start))?;
        Ok(())
    }

    fn get_download_gap(&self, content_length: u64) -> Option<Range<u64>> {
        let range = 0..content_length;
        self.downloaded.next_gap(&range)
    }

    fn signal_download_complete(&self) {
        self.position_reached.notify_stream_done()
    }

    pub(crate) fn source_handle(&self) -> SourceHandle {
        SourceHandle {
            downloaded: self.downloaded.clone(),
            requested_position: self.requested_position.clone(),
            position_reached: self.position_reached.clone(),
            seek_tx: self.seek_tx.clone(),
            content_length: self.content_length,
        }
    }
}
