use std::io::{Read, Seek, SeekFrom};
use std::num::NonZeroUsize;
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::time::Duration;
use std::{fs, io};

use opendal::{Operator, services};
use rstest::rstest;
use setup::{
    ASSETS, Command, ErrorTestStorageProvider, SERVER_RT, TestClient, music_path, server_addr,
};
use stream_download::async_read::AsyncReadStreamParams;
use stream_download::http::{HttpStream, HttpStreamError};
use stream_download::process::{self, ProcessStreamParams};
use stream_download::source::{ContentLength, SourceStream};
use stream_download::storage::StorageProvider;
use stream_download::storage::adaptive::AdaptiveStorageProvider;
use stream_download::storage::bounded::BoundedStorageProvider;
use stream_download::storage::memory::MemoryStorageProvider;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload, StreamInitializationError, StreamState, http};
use stream_download_opendal::{OpendalStream, OpendalStreamParams, StreamDownloadExt};
use tokio::sync::{mpsc, oneshot};
use tokio::task::spawn_blocking;

mod setup;

fn get_file_buf() -> Vec<u8> {
    fs::read(music_path()).unwrap()
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
    SERVER_RT.block_on(async move {
        let mut reader = StreamDownload::new::<http::HttpStream<reqwest::Client>>(
            format!("http://{}/music.mp3", server_addr())
                .parse()
                .unwrap(),
            MemoryStorageProvider,
            Settings::default().prefetch_bytes(prefetch_bytes),
        )
        .await
        .unwrap();

        assert_unwind_safe(&reader);

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
fn new_with_middleware(#[case] prefetch_bytes: u64) {
    use reqwest_retry::RetryTransientMiddleware;
    use reqwest_retry::policies::ExponentialBackoff;

    SERVER_RT.block_on(async move {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        Settings::add_default_middleware(RetryTransientMiddleware::new_with_policy(retry_policy));

        let mut reader = StreamDownload::new_http_with_middleware(
            format!("http://{}/music.mp3", server_addr())
                .parse()
                .unwrap(),
            MemoryStorageProvider,
            Settings::default().prefetch_bytes(prefetch_bytes),
        )
        .await
        .unwrap();

        assert_unwind_safe(&reader);

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
    SERVER_RT.block_on(async move {
        let builder = services::Http::default().endpoint(&format!("http://{}", server_addr()));
        let operator = Operator::new(builder).unwrap().finish();
        let mut reader = StreamDownload::new_opendal(
            OpendalStreamParams::new(operator, "music.mp3")
                .chunk_size(NonZeroUsize::new(chunk_size).unwrap()),
            MemoryStorageProvider,
            Settings::default().prefetch_bytes(prefetch_bytes),
        )
        .await
        .unwrap();

        assert_unwind_safe(&reader);

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
    SERVER_RT.block_on(async move {
        let stream = http::HttpStream::new(
            reqwest::Client::new(),
            format!("http://{}/music.mp3", server_addr())
                .parse()
                .unwrap(),
        )
        .await
        .unwrap();

        let file_buf = get_file_buf();
        let content_length: Option<u64> = stream.content_length().into();
        assert_eq!(file_buf.len() as u64, content_length.unwrap());
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
            MemoryStorageProvider,
            Settings::default().prefetch_bytes(prefetch_bytes),
        )
        .await
        .unwrap();

        assert_unwind_safe(&reader);

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
    SERVER_RT.block_on(async move {
        let builder = services::Http::default().endpoint(&format!("http://{}", server_addr()));
        let operator = Operator::new(builder).unwrap().finish();
        let stream = OpendalStream::new(OpendalStreamParams::new(operator, "music.mp3"))
            .await
            .unwrap();

        let file_buf = get_file_buf();
        let content_length: Option<u64> = stream.content_length().into();
        assert_eq!(file_buf.len() as u64, content_length.unwrap());
        assert_eq!("audio/mpeg", stream.content_type().unwrap());

        let mut reader = StreamDownload::from_stream(
            stream,
            MemoryStorageProvider,
            Settings::default().prefetch_bytes(prefetch_bytes),
        )
        .await
        .unwrap();

        assert_unwind_safe(&reader);

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
    SERVER_RT.block_on(async move {
        let reader = StreamDownload::new_http(
            format!("http://{}/invalid.mp3", server_addr())
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
    SERVER_RT.block_on(async move {
        let mut reader = StreamDownload::new_http(
            format!("http://{}/music.mp3", server_addr())
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
        TempStorageProvider::new_in(ASSETS),
        TempStorageProvider::with_prefix("testfile"),
        TempStorageProvider::with_prefix_in("testfile", ASSETS),
        TempStorageProvider::with_tempfile_builder(|| {
            tempfile::Builder::new().suffix("testfile").tempfile()
        }),
    )]
    storage: TempStorageProvider,
) {
    SERVER_RT.block_on(async move {
        let mut reader = StreamDownload::new_http(
            format!("http://{}/music.mp3", server_addr())
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

#[test]
fn error_on_large_bounded_read() {
    SERVER_RT.block_on(async move {
        let mut reader = StreamDownload::new_http(
            format!("http://{}/music.mp3", server_addr())
                .parse()
                .unwrap(),
            BoundedStorageProvider::new(MemoryStorageProvider, 1024.try_into().unwrap()),
            Settings::default(),
        )
        .await
        .unwrap();

        spawn_blocking(move || {
            let mut buf = vec![0; 2048];
            let res = reader.read(&mut buf);
            assert!(res.is_err());
        })
        .await
        .unwrap();
    });
}

#[test]
fn error_on_large_bounded_seek() {
    SERVER_RT.block_on(async move {
        let mut reader = StreamDownload::new_http(
            format!("http://{}/music.mp3", server_addr())
                .parse()
                .unwrap(),
            BoundedStorageProvider::new(MemoryStorageProvider, 1024.try_into().unwrap()),
            Settings::default(),
        )
        .await
        .unwrap();

        spawn_blocking(move || {
            let res = reader.seek(SeekFrom::Start(2048));
            assert!(res.is_err());
        })
        .await
        .unwrap();
    });
}

#[test]
fn return_error() {
    SERVER_RT.block_on(async move {
        let mut reader = StreamDownload::new_http(
            format!("http://{}/music.mp3", server_addr())
                .parse()
                .unwrap(),
            ErrorTestStorageProvider(MemoryStorageProvider),
            Settings::default(),
        )
        .await
        .unwrap();

        spawn_blocking(move || {
            let mut buf = Vec::new();
            let res = reader.read_to_end(&mut buf);
            assert!(res.is_err());
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
    SERVER_RT.block_on(async move {
        let (tx, mut rx) = mpsc::unbounded_channel::<(Command, oneshot::Sender<Duration>)>();

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
                format!("http://{}/music.mp3", server_addr())
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
fn cancel_on_drop(
    #[values(0, 1, 256*1024, 1024*1024)] prefetch_bytes: u64,
    #[values(MemoryStorageProvider)] storage: impl StorageProvider + 'static,
) {
    SERVER_RT.block_on(async move {
        let (tx, mut rx) = mpsc::unbounded_channel::<(Command, oneshot::Sender<Duration>)>();

        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let (command, responder) = rx.recv().await.unwrap();
            assert_eq!(Command::GetUrl, command);
            responder.send(Duration::from_millis(50)).unwrap();

            while let Some((command, responder)) = rx.recv().await {
                assert!(matches!(command, Command::NextChunk(_)));
                responder.send(Duration::from_millis(50)).unwrap();
            }
        });

        let reader = StreamDownload::from_stream(
            http::HttpStream::new(
                TestClient::new(tx, true),
                format!("http://{}/music.mp3", server_addr())
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
        let reader_handle = reader.handle();
        drop(reader);
        tokio::time::timeout(
            Duration::from_millis(10),
            reader_handle.wait_for_completion(),
        )
        .await
        .unwrap();

        // don't care if this errors because channel send will fail due to stream being dropped
        let _ = handle.await;
    });
}

#[rstest]
fn retry_stuck_download(
    #[values(0, 1, 256*1024, 1024*1024)] prefetch_bytes: u64,
    #[values(TempStorageProvider::default(), MemoryStorageProvider)] storage: impl StorageProvider
    + 'static,
) {
    SERVER_RT.block_on(async move {
        let (tx, mut rx) = mpsc::unbounded_channel::<(Command, oneshot::Sender<Duration>)>();

        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let (command, responder) = rx.recv().await.unwrap();
            assert_eq!(Command::GetUrl, command);
            responder.send(Duration::from_millis(50)).unwrap();

            let mut retry_done = false;
            let mut waiting_for_seek = false;
            let mut got_seek = false;
            while let Some((command, responder)) = rx.recv().await {
                if command == Command::EndStream {
                    assert!(got_seek);
                    return;
                }
                if waiting_for_seek && matches!(command, Command::GetRange) {
                    waiting_for_seek = false;
                    got_seek = true;
                } else {
                    assert!(matches!(command, Command::NextChunk(_)));
                }
                if retry_done {
                    responder.send(Duration::from_millis(50)).unwrap();
                } else {
                    responder.send(Duration::from_millis(200)).unwrap();
                    retry_done = true;
                    waiting_for_seek = true;
                }
            }
            panic!("Stream not finished");
        });

        let mut reader = StreamDownload::from_stream(
            http::HttpStream::new(
                TestClient::new(tx, true),
                format!("http://{}/music.mp3", server_addr())
                    .parse()
                    .unwrap(),
            )
            .await
            .unwrap(),
            storage,
            Settings::default()
                .prefetch_bytes(prefetch_bytes)
                .retry_timeout(Duration::from_millis(100)),
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
fn bounded<T>(
    #[values(0, 1, 128*1024-1, 128*1024)] prefetch_bytes: u64,
    #[values(256*1024, 300*1024)] bounded_length: usize,
    #[values(4096, 128)] batch_write_size: usize,
    #[values(TempStorageProvider::default(), MemoryStorageProvider)] storage: T,
) where
    T: StorageProvider<Reader: RefUnwindSafe + UnwindSafe>,
{
    let buf = SERVER_RT.block_on(async move {
        let (tx, mut rx) = mpsc::unbounded_channel::<(Command, oneshot::Sender<Duration>)>();

        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let (command, responder) = rx.recv().await.unwrap();
            assert_eq!(Command::GetUrl, command);
            responder.send(Duration::from_millis(50)).unwrap();

            let prefetch_size = loop {
                let (command, responder) = rx.recv().await.unwrap();
                if let Command::NextChunk(size) = command
                    && size >= prefetch_bytes as usize
                {
                    responder.send(Duration::from_millis(0)).ok();
                    break size;
                }

                responder.send(Duration::from_millis(0)).ok();
            };
            (rx, prefetch_size)
        });

        let mut reader = StreamDownload::from_stream(
            http::HttpStream::new(
                TestClient::new(tx, false),
                format!("http://{}/music.mp3", server_addr())
                    .parse()
                    .unwrap(),
            )
            .await
            .unwrap(),
            BoundedStorageProvider::new(storage, NonZeroUsize::new(bounded_length).unwrap()),
            Settings::default()
                .prefetch_bytes(prefetch_bytes)
                .batch_write_size(NonZeroUsize::new(batch_write_size).unwrap()),
        )
        .await
        .unwrap();

        assert_unwind_safe(&reader);

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
fn adaptive<T>(
    #[values(0, 1, 128*1024-1, 128*1024)] prefetch_bytes: u64,
    #[values(256, 1024, 128*1024)] buf_size: usize,
    #[values(true, false)] has_content_length: bool,
    #[values(TempStorageProvider::default(), MemoryStorageProvider)] storage: T,
) where
    T: StorageProvider<Reader: RefUnwindSafe + UnwindSafe> + Clone + 'static,
{
    let buf = SERVER_RT.block_on(async move {
        let (tx, mut rx) = mpsc::unbounded_channel::<(Command, oneshot::Sender<Duration>)>();

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
                format!("http://{}/music.mp3", server_addr())
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

        assert_unwind_safe(&reader);

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
    SERVER_RT.block_on(async move {
        let (tx, mut rx) = mpsc::unbounded_channel::<(Command, oneshot::Sender<Duration>)>();

        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let (command, responder) = rx.recv().await.unwrap();
            assert_eq!(Command::GetUrl, command);
            responder.send(Duration::from_millis(50)).unwrap();
            rx
        });

        let bounded_size = 128 * 1024;
        let mut reader = StreamDownload::from_stream(
            http::HttpStream::new(
                TestClient::new(tx, false),
                format!("http://{}/music.mp3", server_addr())
                    .parse()
                    .unwrap(),
            )
            .await
            .unwrap(),
            BoundedStorageProvider::new(
                MemoryStorageProvider,
                NonZeroUsize::new(bounded_size).unwrap(),
            ),
            Settings::default().prefetch_bytes(0),
        )
        .await
        .unwrap();
        let mut rx = handle.await.unwrap();
        let mut prev_size = 0;

        spawn_blocking(move || {
            while let Some((command, responder)) = rx.blocking_recv() {
                responder.send(Duration::from_millis(0)).ok();
                if let Command::NextChunk(size) = command {
                    let buf_size = size - prev_size;
                    let mut temp_buf = vec![0; buf_size];

                    let mut read = 0;
                    loop {
                        let new_read = reader.read(&mut temp_buf).unwrap();
                        read += new_read;
                        if new_read == 0 || read == buf_size {
                            break;
                        }
                    }

                    if size > bounded_size {
                        // After reading past the buffer size, seek to the beginning.
                        // This should produce an error since we're attempting to read past the
                        // available buffer.
                        reader.rewind().unwrap();
                        let mut temp_buf = vec![0; buf_size];
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
        })
        .await
        .unwrap();
    });
}

#[rstest]
fn backpressure(
    #[values(0, 1, 256*1024)] prefetch_bytes: u64,
    #[values(4096, 4096*2+1, 256*1024)] bounded_size: usize,
    #[values(1, 5)] multiplier: usize,
) {
    SERVER_RT.block_on(async move {
        let mut reader = StreamDownload::new_http(
            format!("http://{}/music.mp3", server_addr())
                .parse()
                .unwrap(),
            BoundedStorageProvider::new(
                MemoryStorageProvider,
                NonZeroUsize::new(bounded_size * multiplier).unwrap(),
            ),
            Settings::default().prefetch_bytes(prefetch_bytes),
        )
        .await
        .unwrap();
        tokio::task::spawn_blocking(move || {
            let file_buf = get_file_buf();
            let mut buf = vec![0; file_buf.len()];
            let mut written = 0;
            while written < file_buf.len() {
                let new_written = reader
                    .read(&mut buf[written..(written + (4096 * multiplier)).min(file_buf.len())])
                    .unwrap();
                written += new_written;
            }
            compare(buf, file_buf);
        })
        .await
        .unwrap();
    });
}

#[rstest]
fn seek_basic(
    #[values(0, 1, 256*1024, 1024*1024)] prefetch_bytes: u64,
    #[values(TempStorageProvider::default(), MemoryStorageProvider)] storage: impl StorageProvider
    + 'static,
) {
    SERVER_RT.block_on(async move {
        let (tx, mut rx) = mpsc::unbounded_channel::<(Command, oneshot::Sender<Duration>)>();

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
                format!("http://{}/music.mp3", server_addr())
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
    SERVER_RT.block_on(async move {
        let builder = services::Http::default().endpoint(&format!("http://{}", server_addr()));

        let operator = Operator::new(builder).unwrap().finish();

        let mut reader = StreamDownload::new::<OpendalStream>(
            OpendalStreamParams::new(operator, "music.mp3"),
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
    SERVER_RT.block_on(async move {
        let (tx, mut rx) = mpsc::unbounded_channel::<(Command, oneshot::Sender<Duration>)>();

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
                format!("http://{}/music.mp3", server_addr())
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
    SERVER_RT.block_on(async move {
        let mut reader = StreamDownload::new_http(
            format!("http://{}/music.mp3", server_addr())
                .parse()
                .unwrap(),
            MemoryStorageProvider,
            Settings::default().prefetch_bytes(prefetch_bytes),
        )
        .await
        .unwrap();

        spawn_blocking(move || {
            let mut buf = [0; 1];
            reader.read_exact(&mut buf).unwrap();
            reader.cancel_download();
            std::thread::sleep(Duration::from_millis(10));
            let mut buf = Vec::new();
            let _ = reader.read_to_end(&mut buf);

            let file_buf = get_file_buf();
            assert!(buf.len() < file_buf.len());
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
    let (tx, mut rx) = mpsc::unbounded_channel::<(ContentLength, StreamState)>();

    SERVER_RT.block_on(async move {
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
                    let content_length: Option<u64> = next.0.into();
                    return content_length.unwrap();
                }
            }
        });
        let mut reader = StreamDownload::new_http(
            format!("http://{}/music.mp3", server_addr())
                .parse()
                .unwrap(),
            MemoryStorageProvider,
            Settings::default()
                .prefetch_bytes(prefetch_bytes)
                .on_progress(move |stream: &HttpStream<_>, info, _| {
                    let _ = tx.send((stream.content_length(), info));
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
    let (tx, mut rx) = mpsc::unbounded_channel::<(ContentLength, StreamState)>();

    SERVER_RT.block_on(async move {
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
                    let content_length: Option<u64> = next.0.into();
                    return content_length.unwrap();
                }
            }
        });
        let mut reader = StreamDownload::new_http(
            format!("http://{}/music.mp3", server_addr())
                .parse()
                .unwrap(),
            MemoryStorageProvider,
            Settings::default().prefetch_bytes(0).on_progress(
                move |stream: &HttpStream<_>, info, _| {
                    let _ = tx.send((stream.content_length(), info));
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
    let (tx, mut rx) = mpsc::unbounded_channel::<(ContentLength, StreamState)>();

    SERVER_RT.block_on(async move {
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
                    let content_length: Option<u64> = next.0.into();
                    return content_length.unwrap();
                }
            }
        });
        let mut reader = StreamDownload::new_http(
            format!("http://{}/music.mp3", server_addr())
                .parse()
                .unwrap(),
            MemoryStorageProvider,
            Settings::default()
                .prefetch_bytes(prefetch_bytes)
                .on_progress(move |stream: &HttpStream<_>, info, _| {
                    let _ = tx.send((stream.content_length(), info));
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

#[tokio::test]
async fn async_read_file() {
    let mut reader = StreamDownload::new_async_read(
        AsyncReadStreamParams::new(tokio::fs::File::open(music_path()).await.unwrap()),
        MemoryStorageProvider,
        Settings::default(),
    )
    .await
    .unwrap();

    assert_unwind_safe(&reader);

    spawn_blocking(move || {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).unwrap();
        compare(get_file_buf(), buf);
    })
    .await
    .unwrap();
}

// On Windows, passing raw byte streams between pipes without the shell messing with the encoding
// only works in very specific circumstances.
// Invoking cmd rather than pwsh or powershell is the only thing that seems to work here.
// I'm guessing this has to do with how it detects which programs are "native".
// See https://stackoverflow.com/questions/62835179/how-do-i-pipe-a-byte-stream-to-from-an-external-command
#[tokio::test]
async fn process() {
    let music = music_path();
    let cmd = if cfg!(windows) {
        process::Command::new("cmd").args(["/c", "type", &music.replace('/', "\\")])
    } else {
        process::Command::new("cat").args([music])
    };
    let mut reader = StreamDownload::new_process(
        ProcessStreamParams::new(cmd).unwrap(),
        MemoryStorageProvider,
        Settings::default().cancel_on_drop(false),
    )
    .await
    .unwrap();

    assert_unwind_safe(&reader);

    let handle = reader.handle();
    spawn_blocking(move || {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).unwrap();
        compare(get_file_buf(), buf);
    })
    .await
    .unwrap();
    handle.wait_for_completion().await;
}

// The trick for Windows mentioned above doesn't seem to work with multiple piped commands
#[cfg(not(windows))]
#[tokio::test]
async fn process_piped() {
    let cmd = process::CommandBuilder::new(process::Command::new("cat").arg(music_path()))
        .pipe(process::Command::new("cat"));
    let mut reader = StreamDownload::new_process(
        ProcessStreamParams::new(cmd).unwrap(),
        MemoryStorageProvider,
        Settings::default().cancel_on_drop(false),
    )
    .await
    .unwrap();

    assert_unwind_safe(&reader);

    let handle = reader.handle();
    spawn_blocking(move || {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).unwrap();
        compare(get_file_buf(), buf);
    })
    .await
    .unwrap();
    handle.wait_for_completion().await;
}

fn assert_unwind_safe<T>(_t: &T)
where
    T: RefUnwindSafe + UnwindSafe,
{
}
