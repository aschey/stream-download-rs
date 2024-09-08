use std::io::{Read, Seek, SeekFrom};
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use std::{fs, io};

mod setup;

use bytes::Bytes;
use futures::{Stream, StreamExt};
use opendal::{services, Operator};
use rstest::rstest;
use setup::{SERVER_ADDR, SERVER_RT};
use stream_download::http::{HttpStream, HttpStreamError};
use stream_download::open_dal::{OpenDalStream, OpenDalStreamParams};
use stream_download::source::{DecodeError, SourceStream};
use stream_download::storage::adaptive::AdaptiveStorageProvider;
use stream_download::storage::bounded::BoundedStorageProvider;
use stream_download::storage::memory::MemoryStorageProvider;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::storage::StorageProvider;
use stream_download::{http, Settings, StreamDownload, StreamInitializationError};
use tokio::sync::{mpsc, oneshot};
use tokio::task::spawn_blocking;

#[derive(Debug)]
struct TestClient {
    inner: reqwest::Client,
    tx: mpsc::Sender<(Command, oneshot::Sender<Duration>)>,
    has_content_length: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum Command {
    GetUrl,
    GetRange,
    NextChunk(usize),
    EndStream,
}

struct TestResponse {
    inner: reqwest::Response,
    tx: mpsc::Sender<(Command, oneshot::Sender<Duration>)>,
    has_content_length: bool,
}

enum StreamState {
    Ready,
    Waiting,
}

struct TestStream {
    inner: Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Unpin + Send + Sync>,
    tx: mpsc::Sender<(Command, oneshot::Sender<Duration>)>,
    total_size: usize,
    state: StreamState,
}

impl Stream for TestStream {
    type Item = Result<Bytes, reqwest::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.state {
            StreamState::Ready => {
                let (tx, rx) = oneshot::channel();
                self.tx
                    .try_send((Command::NextChunk(self.total_size), tx))
                    .unwrap();
                let waker = cx.waker().clone();
                self.state = StreamState::Waiting;

                tokio::spawn(async move {
                    tokio::time::sleep(rx.await.unwrap()).await;
                    waker.wake();
                });
                Poll::Pending
            }
            StreamState::Waiting => {
                self.state = StreamState::Ready;
                let mut this = Pin::new(self);
                let res = this.inner.poll_next_unpin(cx);

                match &res {
                    Poll::Ready(None) => {
                        let (tx, _rx) = oneshot::channel();
                        this.tx.try_send((Command::EndStream, tx)).unwrap();
                    }
                    Poll::Ready(Some(Ok(res))) => {
                        this.total_size += res.len();
                        if res.is_empty() {
                            let (tx, _rx) = oneshot::channel();
                            this.tx.try_send((Command::EndStream, tx)).unwrap();
                        }
                    }
                    _ => {}
                };
                res
            }
        }
    }
}

impl TestClient {
    fn new(
        tx: mpsc::Sender<(Command, oneshot::Sender<Duration>)>,
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
        self.tx.send((Command::GetUrl, tx)).await.unwrap();
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
        self.tx.send((Command::GetRange, tx)).await.unwrap();
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
struct TestError(reqwest::Error);

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
            tx: self.tx.clone(),
            inner: self.inner.stream(),
            state: StreamState::Ready,
            total_size: 0,
        })
    }
}

fn get_file_buf() -> Vec<u8> {
    fs::read("./assets/music.mp3").unwrap()
}

fn compare(a: impl Into<Vec<u8>>, b: impl Into<Vec<u8>>) {
    let a = a.into();
    let b = b.into();
    assert_eq!(a.len(), b.len());
    for (i, (l, r)) in a.into_iter().zip(b).enumerate() {
        assert_eq!(l, r, "values differ at position {i}");
    }
}

#[rstest]
#[case(0)]
#[case(1)]
#[case(256*1024)]
#[case(1024*1024)]
fn new(#[case] prefetch_bytes: u64) {
    SERVER_RT.get().unwrap().block_on(async move {
        let mut reader = StreamDownload::new::<http::HttpStream<reqwest::Client>>(
            format!("http://{}/music.mp3", SERVER_ADDR.get().unwrap())
                .parse()
                .unwrap(),
            TempStorageProvider::default(),
            Settings::default().prefetch_bytes(prefetch_bytes),
        )
        .await
        .unwrap();

        spawn_blocking(move || {
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).unwrap();

            compare(get_file_buf(), buf);
        })
        .await
        .unwrap();
    });
}

#[rstest]
#[case(0)]
#[case(1)]
#[case(256*1024)]
#[case(1024*1024)]
fn open_dal_chunk_size(#[case] prefetch_bytes: u64, #[values(745, 1234, 4096)] chunk_size: usize) {
    SERVER_RT.get().unwrap().block_on(async move {
        let builder =
            services::Http::default().endpoint(&format!("http://{}", SERVER_ADDR.get().unwrap()));
        let operator = Operator::new(builder).unwrap().finish();
        let mut reader = StreamDownload::new_open_dal(
            OpenDalStreamParams::new(operator, "music.mp3")
                .chunk_size(NonZeroUsize::new(chunk_size).unwrap()),
            TempStorageProvider::default(),
            Settings::default().prefetch_bytes(prefetch_bytes),
        )
        .await
        .unwrap();

        spawn_blocking(move || {
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).unwrap();

            compare(get_file_buf(), buf);
        })
        .await
        .unwrap();
    });
}

#[rstest]
#[case(0)]
#[case(1)]
#[case(256*1024)]
#[case(1024*1024)]
fn from_stream_http(#[case] prefetch_bytes: u64) {
    SERVER_RT.get().unwrap().block_on(async move {
        let stream = http::HttpStream::new(
            reqwest::Client::new(),
            format!("http://{}/music.mp3", SERVER_ADDR.get().unwrap())
                .parse()
                .unwrap(),
        )
        .await
        .unwrap();

        let file_buf = get_file_buf();
        assert_eq!(file_buf.len() as u64, stream.content_length().unwrap());
        assert_eq!(
            http::ContentType {
                r#type: "audio".to_owned(),
                subtype: "mpeg".to_owned()
            },
            stream.content_type().clone().unwrap()
        );

        assert_eq!("audio/mpeg", stream.header("Content-Type").unwrap());
        assert_eq!("audio/mpeg", stream.headers().get("Content-Type").unwrap());

        let mut reader = StreamDownload::from_stream(
            stream,
            TempStorageProvider::default(),
            Settings::default().prefetch_bytes(prefetch_bytes),
        )
        .await
        .unwrap();

        spawn_blocking(move || {
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).unwrap();

            compare(file_buf, buf);
        })
        .await
        .unwrap();
    });
}

#[rstest]
#[case(0)]
#[case(1)]
#[case(256*1024)]
#[case(1024*1024)]
fn from_stream_open_dal(#[case] prefetch_bytes: u64) {
    SERVER_RT.get().unwrap().block_on(async move {
        let builder =
            services::Http::default().endpoint(&format!("http://{}", SERVER_ADDR.get().unwrap()));
        let operator = Operator::new(builder).unwrap().finish();
        let stream = OpenDalStream::new(OpenDalStreamParams::new(operator, "music.mp3"))
            .await
            .unwrap();

        let file_buf = get_file_buf();
        assert_eq!(file_buf.len() as u64, stream.content_length().unwrap());
        assert_eq!("audio/mpeg", stream.content_type().unwrap());

        let mut reader = StreamDownload::from_stream(
            stream,
            TempStorageProvider::default(),
            Settings::default().prefetch_bytes(prefetch_bytes),
        )
        .await
        .unwrap();

        spawn_blocking(move || {
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).unwrap();

            compare(file_buf, buf);
        })
        .await
        .unwrap();
    });
}

#[rstest]
fn handle_error() {
    SERVER_RT.get().unwrap().block_on(async move {
        let reader = StreamDownload::new_http(
            format!("http://{}/invalid.mp3", SERVER_ADDR.get().unwrap())
                .parse()
                .unwrap(),
            MemoryStorageProvider,
            Settings::default(),
        )
        .await;
        assert!(reader.is_err());
        let err = reader.unwrap_err();
        assert!(matches!(
            err,
            StreamInitializationError::StreamCreationFailure(HttpStreamError::ResponseFailure(_))
        ));
    });
}

#[rstest]
fn basic_download(
    #[values(0, 1, 256*1024, 1024*1024)] prefetch_bytes: u64,
    #[values(TempStorageProvider::default(), MemoryStorageProvider)] storage: impl StorageProvider
    + 'static,
) {
    SERVER_RT.get().unwrap().block_on(async move {
        let mut reader = StreamDownload::new_http(
            format!("http://{}/music.mp3", SERVER_ADDR.get().unwrap())
                .parse()
                .unwrap(),
            storage,
            Settings::default().prefetch_bytes(prefetch_bytes),
        )
        .await
        .unwrap();

        spawn_blocking(move || {
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).unwrap();

            compare(get_file_buf(), buf);
        })
        .await
        .unwrap();
    });
}

#[rstest]
fn tempfile_builder(
    #[values(
        TempStorageProvider::new_in("./assets"),
        TempStorageProvider::with_prefix("testfile"),
        TempStorageProvider::with_prefix_in("testfile", "./assets"),
        TempStorageProvider::with_tempfile_builder(|| {
            tempfile::Builder::new().suffix("testfile").tempfile()
        }),
    )]
    storage: TempStorageProvider,
) {
    SERVER_RT.get().unwrap().block_on(async move {
        let mut reader = StreamDownload::new_http(
            format!("http://{}/music.mp3", SERVER_ADDR.get().unwrap())
                .parse()
                .unwrap(),
            storage,
            Settings::default(),
        )
        .await
        .unwrap();

        spawn_blocking(move || {
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).unwrap();
            compare(get_file_buf(), buf);
        })
        .await
        .unwrap();
    });
}

#[rstest]
fn slow_download(
    #[values(0, 1, 256*1024, 1024*1024)] prefetch_bytes: u64,
    #[values(TempStorageProvider::default(), MemoryStorageProvider)] storage: impl StorageProvider
    + 'static,
) {
    SERVER_RT.get().unwrap().block_on(async move {
        let (tx, mut rx) = mpsc::channel::<(Command, oneshot::Sender<Duration>)>(32);

        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let (command, responder) = rx.recv().await.unwrap();
            assert_eq!(Command::GetUrl, command);
            responder.send(Duration::from_millis(50)).unwrap();

            while let Some((command, responder)) = rx.recv().await {
                if command == Command::EndStream {
                    return;
                }
                assert!(matches!(command, Command::NextChunk(_)));
                responder.send(Duration::from_millis(50)).unwrap();
            }
            panic!("Stream not finished");
        });

        let mut reader = StreamDownload::from_stream(
            http::HttpStream::new(
                TestClient::new(tx, true),
                format!("http://{}/music.mp3", SERVER_ADDR.get().unwrap())
                    .parse()
                    .unwrap(),
            )
            .await
            .unwrap(),
            storage,
            Settings::default().prefetch_bytes(prefetch_bytes),
        )
        .await
        .unwrap();

        spawn_blocking(move || {
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).unwrap();
            compare(get_file_buf(), buf);
        })
        .await
        .unwrap();

        handle.await.unwrap();
    });
}

#[rstest]
fn bounded(
    #[values(0, 1, 128*1024-1, 128*1024)] prefetch_bytes: u64,
    #[values(256*1024, 300*1024)] bounded_length: usize,
    #[values(TempStorageProvider::default(), MemoryStorageProvider)] storage: impl StorageProvider,
) {
    let buf = SERVER_RT.get().unwrap().block_on(async move {
        let (tx, mut rx) = mpsc::channel::<(Command, oneshot::Sender<Duration>)>(32);

        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let (command, responder) = rx.recv().await.unwrap();
            assert_eq!(Command::GetUrl, command);
            responder.send(Duration::from_millis(50)).unwrap();

            let prefetch_size = loop {
                let (command, responder) = rx.recv().await.unwrap();
                if let Command::NextChunk(size) = command {
                    if size >= prefetch_bytes as usize {
                        responder.send(Duration::from_millis(0)).ok();
                        break size;
                    }
                }
                responder.send(Duration::from_millis(0)).ok();
            };
            (rx, prefetch_size as usize)
        });

        let mut reader = StreamDownload::from_stream(
            http::HttpStream::new(
                TestClient::new(tx, false),
                format!("http://{}/music.mp3", SERVER_ADDR.get().unwrap())
                    .parse()
                    .unwrap(),
            )
            .await
            .unwrap(),
            BoundedStorageProvider::new(storage, NonZeroUsize::new(bounded_length).unwrap()),
            Settings::default().prefetch_bytes(prefetch_bytes),
        )
        .await
        .unwrap();

        let (mut rx, prefetch_size) = handle.await.unwrap();

        let mut buf = vec![0; prefetch_size];
        reader.read_exact(&mut buf).unwrap();
        let mut prev_size = prefetch_size;

        while let Some((command, responder)) = rx.recv().await {
            if let Command::NextChunk(size) = command {
                let mut temp_buf = vec![0; size - prev_size];
                reader.read_exact(&mut temp_buf).unwrap();
                buf.extend(&temp_buf);
                prev_size = size;
            }
            responder.send(Duration::from_millis(0)).ok();
        }
        buf
    });
    compare(get_file_buf(), buf);
}

#[rstest]
fn adaptive(
    #[values(0, 1, 128*1024-1, 128*1024)] prefetch_bytes: u64,
    #[values(256, 1024, 128*1024)] buf_size: usize,
    #[values(true, false)] has_content_length: bool,
    #[values(TempStorageProvider::default(), MemoryStorageProvider)] storage: impl StorageProvider
    + 'static,
) {
    let buf = SERVER_RT.get().unwrap().block_on(async move {
        let (tx, mut rx) = mpsc::channel::<(Command, oneshot::Sender<Duration>)>(32);

        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let (command, responder) = rx.recv().await.unwrap();
            assert_eq!(Command::GetUrl, command);
            responder.send(Duration::from_millis(50)).unwrap();

            while let Some((command, responder)) = rx.recv().await {
                if command == Command::EndStream {
                    return;
                }
                assert!(matches!(command, Command::NextChunk(_)));
                responder.send(Duration::from_millis(50)).unwrap();
            }
            panic!("Stream not finished");
        });

        let mut reader = StreamDownload::from_stream(
            http::HttpStream::new(
                TestClient::new(tx, has_content_length),
                format!("http://{}/music.mp3", SERVER_ADDR.get().unwrap())
                    .parse()
                    .unwrap(),
            )
            .await
            .unwrap(),
            AdaptiveStorageProvider::new(storage, NonZeroUsize::new(300 * 1024).unwrap()),
            Settings::default().prefetch_bytes(prefetch_bytes),
        )
        .await
        .unwrap();
        let buf = spawn_blocking(move || {
            let mut buf = Vec::<u8>::new();
            let mut temp_buf = vec![0; buf_size];

            loop {
                let read_len = reader.read(&mut temp_buf).unwrap();
                if read_len == 0 {
                    break;
                }
                buf.extend(&temp_buf[..read_len]);
            }
            buf
        })
        .await
        .unwrap();

        handle.await.unwrap();
        buf
    });

    compare(get_file_buf(), buf);
}

#[rstest]
fn bounded_seek_near_beginning() {
    SERVER_RT.get().unwrap().block_on(async move {
        let (tx, mut rx) = mpsc::channel::<(Command, oneshot::Sender<Duration>)>(32);

        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let (command, responder) = rx.recv().await.unwrap();
            assert_eq!(Command::GetUrl, command);
            responder.send(Duration::from_millis(50)).unwrap();
            rx
        });

        let bounded_size = 256 * 1024;
        let mut reader = StreamDownload::from_stream(
            http::HttpStream::new(
                TestClient::new(tx, false),
                format!("http://{}/music.mp3", SERVER_ADDR.get().unwrap())
                    .parse()
                    .unwrap(),
            )
            .await
            .unwrap(),
            BoundedStorageProvider::new(
                MemoryStorageProvider,
                NonZeroUsize::new(256 * 1024).unwrap(),
            ),
            Settings::default().prefetch_bytes(0),
        )
        .await
        .unwrap();
        let mut rx = handle.await.unwrap();

        let mut prev_size = 0;
        while let Some((command, responder)) = rx.recv().await {
            if let Command::NextChunk(size) = command {
                responder.send(Duration::from_millis(0)).ok();
                let mut temp_buf = vec![0; size - prev_size];
                if size > bounded_size {
                    reader.rewind().unwrap();
                    if let Err(e) = reader.read(&mut temp_buf) {
                        assert_eq!(io::ErrorKind::InvalidInput, e.kind());
                    } else {
                        panic!("should've errored");
                    }
                    return;
                }

                prev_size = size;
            }
        }
    });
}

#[rstest]
fn seek_basic(
    #[values(0, 1, 256*1024, 1024*1024)] prefetch_bytes: u64,
    #[values(TempStorageProvider::default(), MemoryStorageProvider)] storage: impl StorageProvider
    + 'static,
) {
    SERVER_RT.get().unwrap().block_on(async move {
        let (tx, mut rx) = mpsc::channel::<(Command, oneshot::Sender<Duration>)>(32);

        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let (command, responder) = rx.recv().await.unwrap();
            assert_eq!(Command::GetUrl, command);
            responder.send(Duration::from_millis(50)).unwrap();

            while let Some((command, responder)) = rx.recv().await {
                if command == Command::EndStream {
                    return;
                }
                assert!(matches!(command, Command::NextChunk(_)));
                responder.send(Duration::from_millis(50)).unwrap();
            }
            panic!("Stream not finished");
        });

        let mut reader = StreamDownload::from_stream(
            http::HttpStream::new(
                TestClient::new(tx, true),
                format!("http://{}/music.mp3", SERVER_ADDR.get().unwrap())
                    .parse()
                    .unwrap(),
            )
            .await
            .unwrap(),
            storage,
            Settings::default().prefetch_bytes(prefetch_bytes),
        )
        .await
        .unwrap();

        spawn_blocking(move || {
            let mut initial_buf = [0; 4096];
            reader.read_exact(&mut initial_buf).unwrap();
            reader.seek(SeekFrom::Start(0)).unwrap();

            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).unwrap();

            let file_buf = get_file_buf();
            compare(&file_buf[..4096], initial_buf);
            compare(file_buf, buf);
        })
        .await
        .unwrap();

        handle.await.unwrap();
    });
}

#[rstest]
fn seek_basic_open_dal(
    #[values(0, 1, 256*1024, 1024*1024)] prefetch_bytes: u64,
    #[values(TempStorageProvider::default(), MemoryStorageProvider)] storage: impl StorageProvider
    + 'static,
) {
    SERVER_RT.get().unwrap().block_on(async move {
        let builder =
            services::Http::default().endpoint(&format!("http://{}", SERVER_ADDR.get().unwrap()));

        let operator = Operator::new(builder).unwrap().finish();

        let mut reader = StreamDownload::new::<OpenDalStream>(
            OpenDalStreamParams::new(operator, "music.mp3"),
            storage,
            Settings::default().prefetch_bytes(prefetch_bytes),
        )
        .await
        .unwrap();

        spawn_blocking(move || {
            let mut first_read = [0; 4096];
            reader.read_exact(&mut first_read).unwrap();
            let mut second_read = [0; 16];
            let seek_pos = reader.seek(SeekFrom::End(-16)).unwrap();
            let file_buf = get_file_buf();
            assert_eq!(seek_pos, (file_buf.len() - 16) as u64);
            reader.read_exact(&mut second_read).unwrap();
            reader.seek(SeekFrom::Start(0)).unwrap();

            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).unwrap();

            compare(&file_buf[..4096], first_read);
            compare(&file_buf[file_buf.len() - 16..], second_read);
            compare(file_buf, buf);
        })
        .await
        .unwrap();
    });
}

#[rstest]
fn seek_all(
    #[values(0, 1, 256*1024, 1024*1024)] prefetch_bytes: u64,
    #[values("start", "current", "end")] seek_from1: &'static str,
    #[values("start", "current", "end")] seek_from2: &'static str,
    #[values(0, 1, 16, 2048)] seek_from_val1: u64,
    #[values(0, 1, 16, 2048)] seek_from_val2: u64,
    #[values(TempStorageProvider::default(), MemoryStorageProvider)] storage: impl StorageProvider
    + 'static,
) {
    SERVER_RT.get().unwrap().block_on(async move {
        let (tx, mut rx) = mpsc::channel::<(Command, oneshot::Sender<Duration>)>(32);

        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let (command, responder) = rx.recv().await.unwrap();
            assert_eq!(Command::GetUrl, command);
            responder.send(Duration::from_millis(50)).unwrap();

            let mut range_requests = 0;
            let mut stream_ends = 0;
            while let Some((command, responder)) = rx.recv().await {
                if command == Command::GetRange {
                    range_requests += 1;
                    responder.send(Duration::from_millis(50)).unwrap();
                    continue;
                }
                if command == Command::EndStream {
                    stream_ends += 1;
                    continue;
                }
                assert!(matches!(command, Command::NextChunk(_)));
                responder.send(Duration::from_millis(50)).unwrap();
            }

            assert!(range_requests > 0);
            assert!(stream_ends > 0);
        });

        let mut reader = StreamDownload::from_stream(
            http::HttpStream::new(
                TestClient::new(tx, true),
                format!("http://{}/music.mp3", SERVER_ADDR.get().unwrap())
                    .parse()
                    .unwrap(),
            )
            .await
            .unwrap(),
            storage,
            Settings::default().prefetch_bytes(prefetch_bytes),
        )
        .await
        .unwrap();

        spawn_blocking(move || {
            if seek_from1 == "start" {
                reader.seek(SeekFrom::Start(seek_from_val1)).unwrap();
            } else if seek_from1 == "end" {
                reader
                    .seek(SeekFrom::End(-(seek_from_val1 as i64)))
                    .unwrap();
            } else if seek_from1 == "current" {
                reader
                    .seek(SeekFrom::Current(seek_from_val1 as i64))
                    .unwrap();
            }

            let mut buf1 = Vec::new();
            reader.read_to_end(&mut buf1).unwrap();

            if seek_from2 == "start" {
                reader.seek(SeekFrom::Start(seek_from_val2)).unwrap();
            } else if seek_from2 == "end" {
                reader
                    .seek(SeekFrom::End(-(seek_from_val2 as i64)))
                    .unwrap();
            } else if seek_from2 == "current" {
                reader
                    .seek(SeekFrom::Current(-(seek_from_val2 as i64)))
                    .unwrap();
            }

            let mut buf2 = Vec::new();
            reader.read_to_end(&mut buf2).unwrap();

            let file_buf = get_file_buf();

            if seek_from1 == "start" || seek_from1 == "current" {
                compare(&file_buf[seek_from_val1 as usize..], buf1);
            } else if seek_from1 == "end" {
                compare(&file_buf[file_buf.len() - seek_from_val1 as usize..], buf1);
            }

            if seek_from2 == "start" {
                compare(&file_buf[seek_from_val2 as usize..], buf2);
            } else if seek_from2 == "end" || seek_from2 == "current" {
                compare(&file_buf[file_buf.len() - seek_from_val2 as usize..], buf2);
            }
        })
        .await
        .unwrap();

        handle.await.unwrap();
    });
}

#[rstest]
#[case(0)]
#[case(1)]
#[case(256*1024)]
#[case(1024*1024)]
fn cancel_download(#[case] prefetch_bytes: u64) {
    SERVER_RT.get().unwrap().block_on(async move {
        let mut reader = StreamDownload::new_http(
            format!("http://{}/music.mp3", SERVER_ADDR.get().unwrap())
                .parse()
                .unwrap(),
            TempStorageProvider::default(),
            Settings::default().prefetch_bytes(prefetch_bytes),
        )
        .await
        .unwrap();

        spawn_blocking(move || {
            let mut buf = [0; 1];
            reader.read_exact(&mut buf).unwrap();
            reader.cancel_download();

            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).unwrap();

            let file_buf = get_file_buf();
            assert!(!buf.is_empty() && buf.len() < file_buf.len());
            compare(&file_buf[1..buf.len() + 1], buf);
        })
        .await
        .unwrap();
    });
}

#[rstest]
#[case(1)]
#[case(1024)]
fn on_progress(#[case] prefetch_bytes: u64) {
    use stream_download::StreamState;

    let (tx, mut rx) = mpsc::channel::<(Option<u64>, StreamState)>(10000);

    SERVER_RT.get().unwrap().block_on(async move {
        let progress_task = tokio::spawn(async move {
            let next = rx.recv().await.unwrap();
            assert!(matches!(
                next.1.phase,
                stream_download::StreamPhase::Prefetching { .. }
            ));
            loop {
                let next = rx.recv().await.unwrap();
                if !matches!(
                    next.1.phase,
                    stream_download::StreamPhase::Prefetching { .. }
                ) {
                    assert!(matches!(
                        next.1.phase,
                        stream_download::StreamPhase::Downloading { .. }
                    ));
                    break;
                }
            }
            loop {
                let next = rx.recv().await.unwrap();
                if !matches!(
                    next.1.phase,
                    stream_download::StreamPhase::Downloading { .. }
                ) {
                    assert_eq!(next.1.phase, stream_download::StreamPhase::Complete);
                    return next.0.unwrap();
                }
            }
        });
        let mut reader = StreamDownload::new_http(
            format!("http://{}/music.mp3", SERVER_ADDR.get().unwrap())
                .parse()
                .unwrap(),
            TempStorageProvider::default(),
            Settings::default()
                .prefetch_bytes(prefetch_bytes)
                .on_progress(move |stream: &HttpStream<_>, info| {
                    let _ = tx.try_send((stream.content_length(), info));
                }),
        )
        .await
        .unwrap();

        let read_bytes = spawn_blocking(move || {
            let mut read_bytes = 0;
            let mut buf = [0; 4096];
            loop {
                let next = reader.read(&mut buf).unwrap();
                if next == 0 {
                    return read_bytes;
                }
                read_bytes += next;
            }
        });
        let read_bytes = read_bytes.await.unwrap();
        let last_content_length = progress_task.await.unwrap();
        assert_eq!(read_bytes as u64, last_content_length);
    });
}

#[rstest]
fn on_progress_no_prefetch() {
    use stream_download::StreamState;

    let (tx, mut rx) = mpsc::channel::<(Option<u64>, StreamState)>(10000);

    SERVER_RT.get().unwrap().block_on(async move {
        let progress_task = tokio::spawn(async move {
            let next = rx.recv().await.unwrap();
            assert!(matches!(
                next.1.phase,
                stream_download::StreamPhase::Downloading { .. }
            ));
            loop {
                let next = rx.recv().await.unwrap();
                if !matches!(
                    next.1.phase,
                    stream_download::StreamPhase::Downloading { .. }
                ) {
                    assert_eq!(next.1.phase, stream_download::StreamPhase::Complete);
                    return next.0.unwrap();
                }
            }
        });
        let mut reader = StreamDownload::new_http(
            format!("http://{}/music.mp3", SERVER_ADDR.get().unwrap())
                .parse()
                .unwrap(),
            TempStorageProvider::default(),
            Settings::default().prefetch_bytes(0).on_progress(
                move |stream: &HttpStream<_>, info| {
                    let _ = tx.try_send((stream.content_length(), info));
                },
            ),
        )
        .await
        .unwrap();

        let read_bytes = spawn_blocking(move || {
            let mut read_bytes = 0;
            let mut buf = [0; 4096];
            loop {
                let next = reader.read(&mut buf).unwrap();
                if next == 0 {
                    return read_bytes;
                }
                read_bytes += next;
            }
        });
        let read_bytes = read_bytes.await.unwrap();
        let last_content_length = progress_task.await.unwrap();
        assert_eq!(read_bytes as u64, last_content_length);
    });
}

#[rstest]
#[case(512*1024)]
fn on_progress_excessive_prefetch(#[case] prefetch_bytes: u64) {
    use stream_download::StreamState;

    let (tx, mut rx) = mpsc::channel::<(Option<u64>, StreamState)>(10000);

    SERVER_RT.get().unwrap().block_on(async move {
        let progress_task = tokio::spawn(async move {
            let next = rx.recv().await.unwrap();
            assert!(matches!(
                next.1.phase,
                stream_download::StreamPhase::Prefetching { .. }
            ));
            loop {
                let next = rx.recv().await.unwrap();
                if !matches!(
                    next.1.phase,
                    stream_download::StreamPhase::Prefetching { .. },
                ) {
                    assert_eq!(next.1.phase, stream_download::StreamPhase::Complete);
                    return next.0.unwrap();
                }
            }
        });
        let mut reader = StreamDownload::new_http(
            format!("http://{}/music.mp3", SERVER_ADDR.get().unwrap())
                .parse()
                .unwrap(),
            TempStorageProvider::default(),
            Settings::default()
                .prefetch_bytes(prefetch_bytes)
                .on_progress(move |stream: &HttpStream<_>, info| {
                    let _ = tx.try_send((stream.content_length(), info));
                }),
        )
        .await
        .unwrap();

        let read_bytes = spawn_blocking(move || {
            let mut read_bytes = 0;
            let mut buf = [0; 4096];
            loop {
                let next = reader.read(&mut buf).unwrap();
                if next == 0 {
                    return read_bytes;
                }
                read_bytes += next;
            }
        });
        let read_bytes = read_bytes.await.unwrap();
        let last_content_length = progress_task.await.unwrap();
        assert_eq!(read_bytes as u64, last_content_length);
    });
}
