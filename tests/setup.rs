use std::io::{self, Seek, SeekFrom, Write};
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll};
use std::time::Duration;

use axum::Router;
use bytes::Bytes;
use ctor::ctor;
use futures::{Stream, StreamExt};
use stream_download::http;
use stream_download::source::DecodeError;
use stream_download::storage::StorageProvider;
use stream_download::storage::memory::{MemoryStorage, MemoryStorageProvider};
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, oneshot};
use tower_http::services::ServeDir;
use tracing_subscriber::EnvFilter;

pub static SERVER_RT: OnceLock<Runtime> = OnceLock::new();
pub static SERVER_ADDR: OnceLock<SocketAddr> = OnceLock::new();

#[ctor]
fn setup() {
    setup_logger();

    let rt = SERVER_RT.get_or_init(|| Runtime::new().unwrap());
    let _guard = rt.enter();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();

    SERVER_ADDR.get_or_init(|| listener.local_addr().unwrap());
    let service = ServeDir::new("./assets");
    let router = Router::new().fallback_service(service);

    rt.spawn(async move {
        let listener = tokio::net::TcpListener::from_std(listener).unwrap();
        axum::serve(listener, router).await.unwrap();
    });
}

fn setup_logger() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_line_number(true)
        .with_file(true)
        .with_test_writer()
        .init();
}

#[derive(Debug)]
pub struct TestClient {
    inner: reqwest::Client,
    tx: mpsc::UnboundedSender<(Command, oneshot::Sender<Duration>)>,
    has_content_length: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    GetUrl,
    GetRange,
    NextChunk(usize),
    EndStream,
}

pub struct TestResponse {
    inner: reqwest::Response,
    tx: mpsc::UnboundedSender<(Command, oneshot::Sender<Duration>)>,
    has_content_length: bool,
}

struct TestStream {
    inner: Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Unpin + Send + Sync>,
    tx: mpsc::UnboundedSender<(Command, oneshot::Sender<Duration>)>,
    total_size: usize,
    waiting: Arc<AtomicBool>,
    pending: bool,
}

impl Stream for TestStream {
    type Item = Result<Bytes, reqwest::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.waiting.load(Ordering::SeqCst) {
            if self.pending {
                return Poll::Pending;
            }
            let (tx, rx) = oneshot::channel();
            self.tx
                .send((Command::NextChunk(self.total_size), tx))
                .unwrap();
            let waker = cx.waker().clone();
            let waiting = self.waiting.clone();
            self.pending = true;
            tokio::spawn(async move {
                tokio::time::sleep(rx.await.unwrap()).await;
                waiting.store(false, Ordering::SeqCst);
                waker.wake();
            });
            Poll::Pending
        } else {
            self.waiting.store(true, Ordering::SeqCst);
            self.pending = false;
            let mut this = Pin::new(self);
            let res = this.inner.poll_next_unpin(cx);

            match &res {
                Poll::Ready(None) => {
                    let (tx, _rx) = oneshot::channel();
                    this.tx.send((Command::EndStream, tx)).unwrap();
                }
                Poll::Ready(Some(Ok(res))) => {
                    this.total_size += res.len();
                    if res.is_empty() {
                        let (tx, _rx) = oneshot::channel();
                        this.tx.send((Command::EndStream, tx)).unwrap();
                    }
                }
                _ => {}
            };
            res
        }
    }
}

impl TestClient {
    // false positive
    #[allow(dead_code)]
    pub fn new(
        tx: mpsc::UnboundedSender<(Command, oneshot::Sender<Duration>)>,
        has_content_length: bool,
    ) -> Self {
        Self {
            inner: reqwest::Client::new(),
            tx,
            has_content_length,
        }
    }
}

impl http::Client for TestClient {
    type Url = reqwest::Url;
    type Response = TestResponse;
    type Error = reqwest::Error;
    type Headers = reqwest::header::HeaderMap;

    fn create() -> Self {
        unimplemented!()
    }

    async fn get(&self, url: &Self::Url) -> Result<Self::Response, Self::Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.send((Command::GetUrl, tx)).unwrap();
        tokio::time::sleep(rx.await.unwrap()).await;

        http::Client::get(&self.inner, url)
            .await
            .map(|r| TestResponse {
                inner: r,
                tx: self.tx.clone(),
                has_content_length: self.has_content_length,
            })
    }

    async fn get_range(
        &self,
        url: &Self::Url,
        start: u64,
        end: Option<u64>,
    ) -> Result<Self::Response, Self::Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.send((Command::GetRange, tx)).unwrap();
        tokio::time::sleep(rx.await.unwrap()).await;

        Ok(TestResponse {
            inner: self.inner.get_range(url, start, end).await?,
            tx: self.tx.clone(),
            has_content_length: self.has_content_length,
        })
    }
}

#[derive(thiserror::Error, Debug)]
#[error("{0}")]
pub struct TestError(reqwest::Error);

impl DecodeError for TestError {}

impl http::ClientResponse for TestResponse {
    type ResponseError = TestError;
    type StreamError = reqwest::Error;
    type Headers = reqwest::header::HeaderMap;

    fn content_length(&self) -> Option<u64> {
        if self.has_content_length {
            self.inner.content_length()
        } else {
            None
        }
    }

    fn content_type(&self) -> Option<&str> {
        self.inner.content_type()
    }

    fn headers(&self) -> Self::Headers {
        http::ClientResponse::headers(&self.inner)
    }

    fn into_result(self) -> Result<Self, Self::ResponseError> {
        if let Err(error) = self.inner.error_for_status_ref() {
            Err(TestError(error))
        } else {
            Ok(self)
        }
    }

    fn stream(
        self,
    ) -> Box<dyn Stream<Item = Result<Bytes, Self::StreamError>> + Unpin + Send + Sync> {
        Box::new(TestStream {
            pending: false,
            tx: self.tx.clone(),
            inner: self.inner.stream(),
            waiting: Arc::new(AtomicBool::new(true)),
            total_size: 0,
        })
    }
}

#[derive(Clone)]
pub struct ErrorTestStorageProvider(pub MemoryStorageProvider);

pub struct ErrorTestStorage(MemoryStorage);

impl Write for ErrorTestStorage {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::Other, "test error"))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl Seek for ErrorTestStorage {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.0.seek(pos)
    }
}

impl StorageProvider for ErrorTestStorageProvider {
    type Reader = MemoryStorage;
    type Writer = ErrorTestStorage;

    fn into_reader_writer(
        self,
        content_length: Option<u64>,
    ) -> io::Result<(Self::Reader, Self::Writer)> {
        let (reader, writer) = self.0.into_reader_writer(content_length)?;
        Ok((reader, ErrorTestStorage(writer)))
    }
}
