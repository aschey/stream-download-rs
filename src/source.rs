//! Provides the [`SourceStream`] trait which abstracts over the transport used to
//! stream remote content.
use std::error::Error;
use std::fmt;
use std::io::{self, SeekFrom};
use std::ops::Range;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use parking_lot::{Condvar, Mutex, RwLock, RwLockReadGuard};
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
#[async_trait]
pub trait SourceStream:
    Stream<Item = Result<Bytes, Self::StreamError>> + Unpin + Send + Sync + Sized + 'static
{
    /// URL of the remote resource.
    type Url: Send;

    /// Error type thrown by the underlying stream.
    type StreamError: Error + Send;

    /// Creates an instance of the stream.
    async fn create(url: Self::Url) -> io::Result<Self>;

    /// Returns the size of the remote resource in bytes. The result should be `None`
    /// if the stream is infinite or doesn't have a known length.
    fn content_length(&self) -> Option<u64>;

    /// Seeks to a specific position in the stream. This method is only called if the
    /// requested range has not been downloaded, so this method should jump to the
    /// requested position in the stream as quickly as possible.
    async fn seek_range(&mut self, start: u64, end: Option<u64>) -> io::Result<()>;
}

#[derive(PartialEq, Eq)]
enum PrefetchResult {
    Continue,
    Complete,
    EndOfFile,
}

enum DownloadFinishResult {
    Complete,
    ChunkMissing,
}

#[derive(Debug, Clone)]
pub(crate) struct SourceHandle {
    downloaded: Arc<RwLock<RangeSet<u64>>>,
    requested_position: Arc<AtomicI64>,
    position_reached: Arc<(Mutex<Waiter>, Condvar)>,
    content_length: Option<u64>,
    seek_tx: mpsc::Sender<u64>,
}

impl SourceHandle {
    pub fn downloaded(&self) -> RwLockReadGuard<rangemap::RangeSet<u64>> {
        self.downloaded.read()
    }

    pub fn request_position(&self, position: u64) {
        self.requested_position
            .store(position as i64, Ordering::SeqCst);
    }

    pub fn wait_for_requested_position(&self) {
        let (mutex, cvar) = &*self.position_reached;
        let mut waiter = mutex.lock();
        if !waiter.stream_done {
            let wait_start = Instant::now();
            debug!("waiting for requested position");
            cvar.wait_while(&mut waiter, |waiter| {
                !waiter.stream_done && !waiter.position_reached
            });
            if !waiter.stream_done {
                waiter.position_reached = false;
            }
            debug!(
                elapsed = format!("{:?}", wait_start.elapsed()),
                "position reached"
            );
        }
    }

    pub fn seek(&self, position: u64) {
        self.seek_tx.try_send(position).ok();
    }

    pub fn content_length(&self) -> Option<u64> {
        self.content_length
    }
}

#[derive(Default, Debug)]
struct Waiter {
    position_reached: bool,
    stream_done: bool,
}

pub(crate) struct Source<W: StorageWriter> {
    writer: W,
    downloaded: Arc<RwLock<RangeSet<u64>>>,
    requested_position: Arc<AtomicI64>,
    position_reached: Arc<(Mutex<Waiter>, Condvar)>,
    content_length: Option<u64>,
    seek_tx: mpsc::Sender<u64>,
    seek_rx: mpsc::Receiver<u64>,
    settings: Settings,
}

pub struct Download {
    prefetch_complete: bool,
    download_all: bool,
    status: Option<DownloadStatus>,
}

enum DownloadStatus {
    Prefetch(PrefetchResult),
    FinishOrSeek(DownloadFinishResult),
    HandledChunk,
    FetchError,
}

impl Download {
    fn new(settings: &Settings) -> Self {
        Self {
            prefetch_complete: settings.prefetch_bytes == 0,
            download_all: false,
            status: None,
        }
    }
}

impl<H: StorageWriter> Source<H> {
    pub(crate) fn new(writer: H, content_length: Option<u64>, settings: Settings) -> Self {
        let (seek_tx, seek_rx) = mpsc::channel(32);
        let downloaded = Arc::new(RwLock::new(writer.downloaded()));

        Self {
            writer,
            downloaded,
            requested_position: Arc::new(AtomicI64::new(-1)),
            position_reached: Default::default(),
            seek_tx,
            seek_rx,
            content_length,
            settings,
        }
    }

    #[instrument(skip_all)]
    pub(crate) async fn download<S: SourceStream>(
        mut self,
        mut stream: S,
        cancellation_token: CancellationToken,
    ) -> io::Result<()> {
        // Don't start prefetch if it's set to 0
        let mut download = Download::new(&self.settings);
        loop {
            tokio::select! {
                pos = self.seek_rx.recv() => {
                    let pos = pos.expect("one seek_tx kept in self, never drops");
                    if self.should_seek(pos)? {
                        download.prefetch_complete = true;
                        self.seek(&mut stream, pos, None).await?;
                    }
                    continue
                },
                bytes = stream.next() => {
                    download.status = Some(
                    self.handle_bytes(&mut stream,
                         &mut download.prefetch_complete,
                         false, bytes).await?);
                }
                _ = cancellation_token.cancelled() => {
                    self.writer.flush()?;
                    self.signal_download_complete();
                    return Ok(())
                }
            };

            use DownloadFinishResult as DR;
            use DownloadStatus as DS;
            use PrefetchResult as PR;

            match download.status.as_ref().unwrap() {
                DS::Prefetch(PR::Continue) => continue,
                DS::Prefetch(PR::Complete) => continue,
                DS::Prefetch(PR::EndOfFile) => break,
                DS::FinishOrSeek(DR::Complete) => break,
                DS::FinishOrSeek(DR::ChunkMissing) => continue,
                DS::HandledChunk => continue,
                DS::FetchError => continue,
            }
        }
        Ok(())
    }

    /// returns true if the stream is done downloading
    async fn handle_bytes<S: SourceStream, E: fmt::Debug>(
        &mut self,
        stream: &mut S,
        prefetch_complete: &mut bool,
        download_all: bool,
        bytes: Option<Result<Bytes, E>>,
    ) -> io::Result<DownloadStatus> {
        let bytes = match bytes.transpose() {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Error fetching chunk from stream: {e:?}");
                return Ok(DownloadStatus::FetchError);
            }
        };

        if let Some(bytes) = bytes {
            if !*prefetch_complete {
                self.writer.write_all(&bytes)?;
                self.writer.flush()?;
                let new_stream_position = self.writer.stream_position()?;
                if new_stream_position >= self.settings.prefetch_bytes {
                    self.downloaded.write().insert(0..new_stream_position);
                    *prefetch_complete = true;
                    Ok(DownloadStatus::Prefetch(PrefetchResult::Complete))
                } else {
                    Ok(DownloadStatus::Prefetch(PrefetchResult::Continue))
                }
            } else {
                self.handle_response_chunk(bytes)?;
                Ok(DownloadStatus::HandledChunk)
            }
        } else { // end of stream
            if !*prefetch_complete { 
                self.downloaded
                    .write()
                    .insert(0..self.writer.stream_position()?);
            }
            let res = self.finish_or_seek(stream, self.content_length).await?;
            Ok(DownloadStatus::FinishOrSeek(res))
        }
    }

    async fn finish_or_seek<S: SourceStream>(
        &mut self,
        stream: &mut S,
        content_length: Option<u64>,
    ) -> io::Result<DownloadFinishResult> {
        if let Some(content_length) = content_length {
            let gap = self.get_download_gap(content_length);
            if let Some(gap) = gap {
                debug!(
                    missing = format!("{gap:?}"),
                    "downloading missing stream chunk"
                );
                self.seek(stream, gap.start, Some(gap.end)).await?;
                return Ok(DownloadFinishResult::ChunkMissing);
            }
        }
        self.writer.flush()?;
        self.signal_download_complete();
        Ok(DownloadFinishResult::Complete)
    }

    fn handle_response_chunk(&mut self, bytes: Bytes) -> io::Result<()> {
        let position = self.writer.stream_position()?;
        self.writer.write_all(&bytes)?;
        self.writer.flush()?;
        let new_position = self.writer.stream_position()?;

        // RangeSet will panic if we try to insert a slice with 0 length. This could
        // happen if the current chunk is empty.
        if new_position > position {
            self.downloaded.write().insert(position..new_position);
        }
        let requested = self.requested_position.load(Ordering::SeqCst);
        if requested > -1 {
            if new_position as i64 >= requested {
                self.requested_position.store(-1, Ordering::SeqCst);
                let (mutex, cvar) = &*self.position_reached;
                (mutex.lock()).position_reached = true;
                cvar.notify_all();
            }
        }
        Ok(())
    }

    fn should_seek(&mut self, pos: u64) -> io::Result<bool> {
        let downloaded = self.downloaded.read();
        Ok(if let Some(range) = downloaded.get(&pos) {
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
        let downloaded = self.downloaded.read();
        let range = 0..content_length;
        let mut gaps = downloaded.gaps(&range);
        gaps.next()
    }

    fn signal_download_complete(&self) {
        let (mutex, cvar) = &*self.position_reached;
        (mutex.lock()).stream_done = true;
        cvar.notify_all();
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
