use async_trait::async_trait;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use parking_lot::{Condvar, Mutex, RwLock, RwLockReadGuard};
use rangemap::RangeSet;
use std::{
    error::Error,
    fs::File,
    io::{BufWriter, Seek, SeekFrom, Write},
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc,
    },
};
use tokio::sync::mpsc;
use tracing::{debug, info, trace};

#[async_trait]
pub trait SourceStream:
    Stream<Item = Result<Bytes, Self::Error>> + Unpin + Send + Sync + 'static
{
    type Url: Send;
    type Error: Error + Send;

    async fn create(url: Self::Url) -> Self;
    async fn content_length(&self) -> Option<u64>;
    async fn seek(&mut self, position: u64);
}

#[derive(Debug, Clone)]
pub struct SourceHandle {
    downloaded: Arc<RwLock<RangeSet<u64>>>,
    requested_position: Arc<AtomicI64>,
    position_reached: Arc<(Mutex<Waiter>, Condvar)>,
    content_length_retrieved: Arc<(Mutex<bool>, Condvar)>,
    content_length: Arc<AtomicI64>,
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
            debug!("Waiting for requested position");
            cvar.wait_while(&mut waiter, |waiter| {
                !waiter.stream_done && !waiter.position_reached
            });
            if !waiter.stream_done {
                waiter.position_reached = false;
            }

            debug!("Position reached");
        }
    }

    pub fn seek(&self, position: u64) {
        self.seek_tx.try_send(position).ok();
    }

    pub fn content_length(&self) -> Option<u64> {
        let (mutex, cvar) = &*self.content_length_retrieved;
        let mut done = mutex.lock();
        if !*done {
            cvar.wait_while(&mut done, |done| !*done);
        }
        let length = self.content_length.load(Ordering::SeqCst);
        if length > -1 {
            Some(length as u64)
        } else {
            None
        }
    }
}

#[derive(Default, Debug)]
struct Waiter {
    position_reached: bool,
    stream_done: bool,
}

pub struct Source {
    writer: BufWriter<File>,
    downloaded: Arc<RwLock<RangeSet<u64>>>,
    position: u64,
    requested_position: Arc<AtomicI64>,
    position_reached: Arc<(Mutex<Waiter>, Condvar)>,
    content_length_retrieved: Arc<(Mutex<bool>, Condvar)>,
    content_length: Arc<AtomicI64>,
    seek_tx: mpsc::Sender<u64>,
    seek_rx: mpsc::Receiver<u64>,
}

const PREFETCH_BYTES: u64 = 1024 * 256;

impl Source {
    pub fn new(tempfile: File) -> Self {
        let (seek_tx, seek_rx) = mpsc::channel(32);
        Self {
            writer: BufWriter::new(tempfile),
            downloaded: Default::default(),
            position: Default::default(),
            requested_position: Arc::new(AtomicI64::new(-1)),
            position_reached: Default::default(),
            content_length_retrieved: Default::default(),
            seek_tx,
            seek_rx,
            content_length: Default::default(),
        }
    }

    pub async fn download<S: SourceStream>(mut self, mut stream: S) {
        info!("Starting file download");
        let content_length = stream.content_length().await;
        if let Some(content_length) = content_length {
            self.content_length
                .swap(content_length as i64, Ordering::SeqCst);
        } else {
            self.content_length.swap(-1, Ordering::SeqCst);
        }

        {
            let (mutex, cvar) = &*self.content_length_retrieved;
            *mutex.lock() = true;
            cvar.notify_all();
        }

        let mut initial_buffer = 0;
        loop {
            if let Some(bytes) = stream.next().await {
                let bytes = bytes.unwrap();
                self.writer.write_all(&bytes).unwrap();
                initial_buffer += bytes.len() as u64;
                trace!("Prefetch: {}/{} bytes", initial_buffer, PREFETCH_BYTES);
                if initial_buffer >= PREFETCH_BYTES {
                    self.position += initial_buffer;
                    self.downloaded.write().insert(0..initial_buffer);
                    break;
                }
            } else {
                info!("File shorter than prefetch length");
                self.writer.flush().unwrap();
                self.position += initial_buffer;
                self.downloaded.write().insert(0..initial_buffer);
                let (mutex, cvar) = &*self.position_reached;
                (mutex.lock()).stream_done = true;
                cvar.notify_all();
                return;
            }
        }

        info!("Prefetch complete");
        loop {
            tokio::select! {
                bytes = stream.next() => {
                    if let Some(bytes) = bytes {
                        let bytes = bytes.unwrap();
                        let chunk_len = bytes.len() as u64;
                        self.writer.write_all(&bytes).unwrap();
                        let new_position = self.position + chunk_len;

                        trace!("Received response chunk. position={}", new_position);
                        self.downloaded.write().insert(self.position..new_position);
                        let requested = self.requested_position.load(Ordering::SeqCst);
                        if requested > -1 {
                            debug!("downloader: requested {requested} current {}", new_position);
                        }

                        if requested > -1 && new_position as i64 >= requested {
                            info!("Notifying");
                            self.requested_position.store(-1, Ordering::SeqCst);
                            let (mutex, cvar) = &*self.position_reached;
                            (mutex.lock()).position_reached = true;
                            cvar.notify_all();
                        }
                        self.position = new_position;
                    } else {
                        info!("Stream finished downloading");
                        self.writer.flush().unwrap();
                        let (mutex, cvar) = &*self.position_reached;
                        (mutex.lock()).stream_done = true;
                        cvar.notify_all();
                        return;
                    }
                },
                pos = self.seek_rx.recv() => {
                    if let Some(pos) = pos {
                        debug!("Received seek position {pos}");
                        let do_seek = {
                            let downloaded = self.downloaded.read();
                            if let Some(range) = downloaded.get(&pos) {
                                !range.contains(&self.position)
                            } else {
                                true
                            }
                        };

                        if do_seek {
                            stream.seek(pos).await;
                            self.writer.seek(SeekFrom::Start(pos)).unwrap();
                            self.position = pos;
                        }
                    }
                }
            }
        }
    }

    pub fn source_handle(&self) -> SourceHandle {
        SourceHandle {
            downloaded: self.downloaded.clone(),
            requested_position: self.requested_position.clone(),
            position_reached: self.position_reached.clone(),
            seek_tx: self.seek_tx.clone(),
            content_length_retrieved: self.content_length_retrieved.clone(),
            content_length: self.content_length.clone(),
        }
    }
}
