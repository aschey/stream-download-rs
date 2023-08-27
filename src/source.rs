//! Provides the [SourceStream] trait which abstracts over the transport used to
//! stream remote content.
use std::error::Error;
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

impl<H: StorageWriter> Source<H> {
    pub(crate) fn new(writer: H, content_length: Option<u64>, settings: Settings) -> Self {
        let (seek_tx, seek_rx) = mpsc::channel(32);
        Self {
            writer,
            downloaded: Default::default(),
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
        debug!("starting file download");

        let download_start = Instant::now();

        // Don't start prefetch if it's set to 0
        let mut prefetch_complete = self.settings.prefetch_bytes == 0;
        loop {
            tokio::select! {
                bytes = stream.next() => {
                    let bytes = match bytes {
                        Some(Err(e)) => {
                            error!("Error fetching chunk from stream: {e:?}");
                            continue;
                        },
                        Some(Ok(bytes)) => {
                            trace!(chunk_size=bytes.len());
                            Some(bytes)
                        },
                        None => None,
                    };

                    if prefetch_complete {
                        if let Some(bytes) = bytes {
                            self.handle_response_chunk(bytes)?;
                        } else {
                            debug!(
                                download_duration = format!("{:?}", download_start.elapsed()),
                                "stream finished downloading"
                            );
                            match self.download_finish(&mut stream, self.content_length).await? {
                                DownloadFinishResult::ChunkMissing => {
                                    continue;
                                },
                                DownloadFinishResult::Complete => {
                                    return Ok(());
                                },
                            }
                        }
                    } else {
                        match self.prefetch(bytes).await? {
                            PrefetchResult::Continue => { },
                            PrefetchResult::Complete => {
                                debug!(
                                    duration = format!("{:?}",
                                    download_start.elapsed()),
                                    "prefetch complete"
                                );
                                prefetch_complete = true;
                            },
                            PrefetchResult::EndOfFile => {
                                debug!(
                                    download_duration = format!("{:?}", download_start.elapsed()),
                                    "stream finished downloading"
                                );
                                return Ok(());
                            },
                        }
                    }
                },
                pos = self.seek_rx.recv() => {
                    if let Some(pos) = pos {
                        debug!(position = pos, "received seek position");
                        if self.should_seek(pos)? {
                            debug!("seek position not yet downloaded");
                            if !prefetch_complete {
                                debug!("seeking during prefetch, ending prefetch early");
                                prefetch_complete = true;
                            }

                            self.seek(&mut stream, pos, None).await?;
                        }
                    }
                },
                _ = cancellation_token.cancelled() => {
                    debug!("received cancellation request, stopping download task");
                    self.writer.flush()?;
                    self.complete_download();
                    return Ok(());
                }
            }
        }
    }

    async fn prefetch(&mut self, bytes: Option<Bytes>) -> io::Result<PrefetchResult> {
        if let Some(bytes) = bytes {
            self.writer.write_all(&bytes)?;
            self.writer.flush()?;
            let stream_position = self.writer.stream_position()?;
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
                self.downloaded.write().insert(0..stream_position);
                Ok(PrefetchResult::Complete)
            } else {
                Ok(PrefetchResult::Continue)
            }
        } else {
            debug!("file shorter than prefetch length, download finished");
            self.writer.flush()?;
            self.downloaded
                .write()
                .insert(0..self.writer.stream_position()?);
            self.complete_download();
            Ok(PrefetchResult::EndOfFile)
        }
    }

    async fn download_finish<S: SourceStream>(
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
        self.complete_download();
        Ok(DownloadFinishResult::Complete)
    }

    fn handle_response_chunk(&mut self, bytes: Bytes) -> io::Result<()> {
        let position = self.writer.stream_position()?;
        self.writer.write_all(&bytes)?;
        self.writer.flush()?;
        let new_position = self.writer.stream_position()?;
        trace!(
            previous_position = position,
            new_position,
            "received response chunk"
        );

        // RangeSet will panic if we try to insert a slice with 0 length. This could
        // happen if the current chunk is empty.
        if new_position > position {
            self.downloaded.write().insert(position..new_position);
        }
        let requested = self.requested_position.load(Ordering::SeqCst);
        if requested > -1 {
            debug!(
                requested_position = requested,
                current_position = new_position,
                "received requested position"
            );
            if new_position as i64 >= requested {
                debug!("requested position reached, notifying");
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

    fn complete_download(&self) {
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
