//! Provides the [`SourceStream`] trait which abstracts over the transport used to
//! stream remote content.
use std::error::Error;
use std::fmt::Debug;
use std::future;
use std::io::{self, SeekFrom};
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::Instant;

use bytes::{BufMut, Bytes, BytesMut};
use futures::{Future, Stream, StreamExt, TryStream};
use parking_lot::{Condvar, Mutex, RwLock};
use rangemap::RangeSet;
use tap::TapFallible;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{Notify, mpsc};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, instrument, trace};

use crate::storage::StorageWriter;
use crate::{CallbackFn, Settings, StreamPhase, StreamState};

/// Represents a remote resource that can be streamed over the network. Streaming
/// over http is implemented via the [`HttpStream`](crate::http::HttpStream)
/// implementation if the `http` feature is enabled.
///
/// The implementation must also implement the
/// [Stream](https://docs.rs/futures/latest/futures/stream/trait.Stream.html) trait.
pub trait SourceStream:
    TryStream<Ok = Bytes>
    + Stream<Item = Result<Self::Ok, Self::Error>>
    + Unpin
    + Send
    + Sync
    + Sized
    + 'static
{
    /// Parameters used to create the remote resource.
    type Params: Send;

    /// Error type thrown when creating the stream.
    type StreamCreationError: DecodeError + Send;

    /// Creates an instance of the stream.
    fn create(
        params: Self::Params,
    ) -> impl Future<Output = Result<Self, Self::StreamCreationError>> + Send;

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
    ) -> impl Future<Output = Result<(), io::Error>> + Send;
}

/// Trait for decoding extra error information asynchronously.
pub trait DecodeError: Error + Send + Sized {
    /// Decode extra error information.
    fn decode_error(self) -> impl Future<Output = String> + Send {
        future::ready(self.to_string())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SourceHandle {
    downloaded: Downloaded,
    requested_position: RequestedPosition,
    position_reached: PositionReached,
    content_length: Option<u64>,
    seek_tx: mpsc::Sender<u64>,
    notify_read: NotifyRead,
}

impl SourceHandle {
    pub(crate) fn get_downloaded_at_position(&self, position: u64) -> Option<Range<u64>> {
        self.downloaded.get(position)
    }

    pub(crate) fn request_position(&self, position: u64) {
        self.requested_position.set(position);
    }

    pub(crate) fn wait_for_requested_position(&self) {
        self.position_reached.wait_for_position_reached();
    }

    pub(crate) fn notify_read(&self) {
        self.notify_read.notify_read();
    }

    pub(crate) fn notify_waiting(&self) {
        self.notify_read.notify_waiting();
    }

    pub(crate) fn seek(&self, position: u64) {
        self.seek_tx
            .try_send(position)
            .tap_err(|e| {
                if let TrySendError::Full(capacity) = e {
                    error!("Seek buffer full. Capacity: {capacity}");
                }
            })
            .ok();
    }

    pub(crate) fn content_length(&self) -> Option<u64> {
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

#[derive(Default, Debug)]
struct Waiter {
    position_reached: bool,
    stream_done: bool,
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

        let wait_start = Instant::now();

        cvar.wait_while(&mut waiter, |waiter| {
            !waiter.stream_done && !waiter.position_reached
        });
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
            self.0.write().insert(range);
        }
    }

    fn get(&self, position: u64) -> Option<Range<u64>> {
        self.0.read().get(&position).cloned()
    }

    fn next_gap(&self, range: &Range<u64>) -> Option<Range<u64>> {
        self.0.read().gaps(range).next()
    }
}

#[derive(Clone, Debug, Default)]
struct NotifyRead {
    notify: Arc<Notify>,
    write_requested: Arc<AtomicBool>,
}

impl NotifyRead {
    fn request(&self) {
        self.write_requested.store(true, Ordering::SeqCst);
    }

    fn notify_read(&self) {
        if self.write_requested.swap(false, Ordering::SeqCst) {
            self.notify.notify_one();
        }
    }

    fn notify_waiting(&self) {
        if self.write_requested.load(Ordering::SeqCst) {
            self.notify.notify_one();
        }
    }

    async fn wait_for_read(&self) {
        self.notify.notified().await;
    }
}

#[derive(PartialEq, Eq)]
enum DownloadStatus {
    Continue,
    Complete,
}

pub(crate) struct Source<S: SourceStream, W: StorageWriter> {
    writer: W,
    downloaded: Downloaded,
    requested_position: RequestedPosition,
    position_reached: PositionReached,
    notify_read: NotifyRead,
    content_length: Option<u64>,
    seek_tx: mpsc::Sender<u64>,
    seek_rx: mpsc::Receiver<u64>,
    prefetch_bytes: u64,
    on_progress: Option<CallbackFn<S>>,
    prefetch_complete: bool,
    prefetch_start_position: u64,
    remaining_bytes: Option<Bytes>,
}

impl<S: SourceStream, H: StorageWriter> Source<S, H>
where
    S::Error: Debug,
{
    pub(crate) fn new(writer: H, content_length: Option<u64>, settings: Settings<S>) -> Self {
        let (seek_tx, seek_rx) = mpsc::channel(settings.seek_buffer_size);
        Self {
            writer,
            downloaded: Downloaded::default(),
            requested_position: RequestedPosition::default(),
            position_reached: PositionReached::default(),
            notify_read: NotifyRead::default(),
            seek_tx,
            seek_rx,
            content_length,
            prefetch_complete: settings.prefetch_bytes == 0,
            prefetch_bytes: settings.prefetch_bytes,
            on_progress: settings.on_progress,
            prefetch_start_position: 0,
            remaining_bytes: None,
        }
    }

    #[instrument(skip_all)]
    pub(crate) async fn download(
        mut self,
        mut stream: S,
        cancellation_token: CancellationToken,
    ) -> io::Result<()> {
        debug!("starting file download");
        let download_start = std::time::Instant::now();

        loop {
            tokio::select! {
                position = self.seek_rx.recv() => {
                    let position = position.expect("seek_tx dropped");
                    if self.should_seek(position)? {
                        debug!("seek position not yet downloaded");

                        if self.prefetch_complete {
                            debug!("re-starting prefetch");
                            self.prefetch_start_position = position;
                            self.prefetch_complete = false;
                        }
                        else {
                            debug!("seeking during prefetch, ending prefetch early");
                            self.downloaded
                                .add(self.prefetch_start_position..self.writer.stream_position()?);
                            self.prefetch_complete = true;
                        }
                        self.seek(&mut stream, position, None).await?;
                    }
                    continue
                },
                bytes = stream.next() => {
                    if self.handle_bytes(&mut stream, bytes, download_start).await?
                    == DownloadStatus::Complete {
                        debug!(
                            download_duration = format!("{:?}", download_start.elapsed()),
                            "stream finished downloading"
                        );
                        break;
                    }
                }
                () = cancellation_token.cancelled() => {
                    debug!("received cancellation request, stopping download task");
                    self.writer.flush()?;
                    self.signal_download_complete();
                    break;
                }
            };
        }
        self.report_download_complete(&stream, download_start)?;
        Ok(())
    }

    async fn handle_prefetch(
        &mut self,
        stream: &mut S,
        bytes: Option<Bytes>,
        start_position: u64,
        download_start: Instant,
    ) -> io::Result<DownloadStatus> {
        let Some(bytes) = bytes else {
            self.prefetch_complete = true;
            debug!("file shorter than prefetch length, download finished");
            self.writer.flush()?;
            let position = self.writer.stream_position()?;
            self.downloaded.add(start_position..position);

            return self.finish_or_find_next_gap(stream).await;
        };
        let written = self.writer.write(&bytes)?;
        self.writer.flush()?;

        let stream_position = self.writer.stream_position()?;
        let partial_write = written < bytes.len();

        // End prefetch early if we weren't able to write the entire contents
        if partial_write {
            debug!(
                written,
                bytes_len = bytes.len(),
                "failed to write all during prefetch"
            );
            self.remaining_bytes = Some(bytes.slice(written..));
        }
        if (stream_position >= start_position + self.prefetch_bytes) || partial_write {
            self.downloaded.add(start_position..stream_position);
            debug!("prefetch complete");
            self.prefetch_complete = true;
        }

        self.report_prefetch_progress(stream, stream_position, download_start, written);
        Ok(DownloadStatus::Continue)
    }

    async fn finish_or_find_next_gap(&mut self, stream: &mut S) -> io::Result<DownloadStatus> {
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

    async fn handle_bytes(
        &mut self,
        stream: &mut S,
        bytes: Option<Result<Bytes, S::Error>>,
        download_start: Instant,
    ) -> io::Result<DownloadStatus> {
        let bytes = match bytes.transpose() {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Error fetching chunk from stream: {e:?}");
                return Ok(DownloadStatus::Continue);
            }
        };

        if !self.prefetch_complete {
            return self
                .handle_prefetch(stream, bytes, self.prefetch_start_position, download_start)
                .await;
        }

        let bytes = match (self.remaining_bytes.take(), bytes) {
            (Some(remaining), Some(bytes)) => {
                let mut combined = BytesMut::new();
                combined.put(remaining);
                combined.put(bytes);
                combined.freeze()
            }
            (Some(remaining), None) => remaining,
            (None, Some(bytes)) => bytes,
            (None, None) => {
                return self.finish_or_find_next_gap(stream).await;
            }
        };
        let bytes_len = bytes.len();
        let new_position = self.write(bytes).await?;
        self.report_downloading_progress(stream, new_position, download_start, bytes_len)?;

        Ok(DownloadStatus::Continue)
    }

    async fn write(&mut self, bytes: Bytes) -> io::Result<u64> {
        let mut written = 0;
        let position = self.writer.stream_position()?;
        let mut new_position = position;
        // Keep writing until we process the whole buffer.
        // If the reader is falling behind, this may take several attempts.
        while written < bytes.len() {
            self.notify_read.request();
            let new_written = self.writer.write(&bytes[written..])?;
            trace!(written, new_written, len = bytes.len(), "wrote data");

            if new_written > 0 {
                self.writer.flush()?;
                written += new_written;
            }
            new_position = self.writer.stream_position()?;
            if new_position > position {
                self.downloaded.add(position..new_position);
            }

            if let Some(requested) = self.requested_position.get() {
                debug!(
                    requested_position = requested,
                    current_position = new_position,
                    "received requested position"
                );

                if new_position >= requested {
                    debug!("notifying position reached");
                    self.requested_position.clear();
                    self.position_reached.notify_position_reached();
                }
            }
            if new_written == 0 {
                // We're not able to write any data, so we need to wait for space to be available
                debug!("waiting for next read");
                self.notify_read.wait_for_read().await;
                debug!("read finished");
            }

            trace!(
                previous_position = position,
                new_position,
                chunk_size = bytes.len(),
                "received response chunk"
            );
        }
        Ok(new_position)
    }

    fn should_seek(&mut self, position: u64) -> io::Result<bool> {
        Ok(if let Some(range) = self.downloaded.get(position) {
            !range.contains(&self.writer.stream_position()?)
        } else {
            true
        })
    }

    async fn seek(&mut self, stream: &mut S, start: u64, end: Option<u64>) -> io::Result<()> {
        stream.seek_range(start, end).await?;
        self.writer.seek(SeekFrom::Start(start))?;
        Ok(())
    }

    fn get_download_gap(&self, content_length: u64) -> Option<Range<u64>> {
        let range = 0..content_length;
        self.downloaded.next_gap(&range)
    }

    fn signal_download_complete(&self) {
        self.position_reached.notify_stream_done();
    }

    fn report_progress(&mut self, stream: &S, info: StreamState) {
        if let Some(on_progress) = self.on_progress.as_mut() {
            on_progress(stream, info);
        }
    }

    fn report_prefetch_progress(
        &mut self,
        stream: &S,
        stream_position: u64,
        download_start: Instant,
        chunk_size: usize,
    ) {
        self.report_progress(stream, StreamState {
            current_position: stream_position,
            current_chunk: (0..stream_position),
            elapsed: download_start.elapsed(),
            phase: StreamPhase::Prefetching {
                target: self.prefetch_bytes,
                chunk_size,
            },
        });
    }

    fn report_downloading_progress(
        &mut self,
        stream: &S,
        new_position: u64,
        download_start: Instant,
        chunk_size: usize,
    ) -> io::Result<()> {
        let pos = self.writer.stream_position()?;
        self.report_progress(stream, StreamState {
            current_position: pos,
            current_chunk: self
                .downloaded
                .get(new_position - 1)
                .expect("position already downloaded"),
            elapsed: download_start.elapsed(),
            phase: StreamPhase::Downloading { chunk_size },
        });
        Ok(())
    }

    fn report_download_complete(&mut self, stream: &S, download_start: Instant) -> io::Result<()> {
        let pos = self.writer.stream_position()?;
        self.report_progress(stream, StreamState {
            current_position: pos,
            elapsed: download_start.elapsed(),
            current_chunk: self.downloaded.get(pos - 1).expect(""),
            phase: StreamPhase::Complete,
        });
        Ok(())
    }

    pub(crate) fn source_handle(&self) -> SourceHandle {
        SourceHandle {
            downloaded: self.downloaded.clone(),
            requested_position: self.requested_position.clone(),
            notify_read: self.notify_read.clone(),
            position_reached: self.position_reached.clone(),
            seek_tx: self.seek_tx.clone(),
            content_length: self.content_length,
        }
    }
}
