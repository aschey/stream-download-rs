# stream-download-rs

[![crates.io](https://img.shields.io/crates/v/stream-download.svg?logo=rust)](https://crates.io/crates/stream-download)
[![docs.rs](https://img.shields.io/docsrs/stream-download?logo=rust)](https://docs.rs/stream-download)
[![Dependency Status](https://deps.rs/repo/github/aschey/stream-download-rs/status.svg?style=flat-square)](https://deps.rs/repo/github/aschey/stream-download-rs)
![license](https://img.shields.io/badge/License-MIT%20or%20Apache%202-green.svg)
[![CI](https://github.com/aschey/stream-download-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/aschey/stream-download-rs/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/aschey/stream-download-rs/branch/main/graph/badge.svg?token=Wx7OgIb0qa)](https://app.codecov.io/gh/aschey/stream-download-rs)
![GitHub repo size](https://img.shields.io/github/repo-size/aschey/stream-download-rs)
![Lines of Code](https://aschey.tech/tokei/github/aschey/stream-download-rs)

[stream-download](https://github.com/aschey/stream-download-rs) is a library for streaming content from a remote location to a local cache and using it as a [`read`](https://doc.rust-lang.org/stable/std/io/trait.Read.html) and [`seek`](https://doc.rust-lang.org/stable/std/io/trait.Seek.html)-able source.
The requested content is downloaded in the background and read or seek operations are allowed before the download is finished.
Seek operations may cause the stream to be restarted from the requested position if the download is still in progress.
This is useful for media applications that need to stream large files that may take a long time to download.

This library makes heavy use of the adapter pattern to allow for pluggable transports and storage implementations.

## Installation

```sh
cargo add stream-download
```

## Features

- `http` - adds an HTTP-based implementation of the [`SourceStream`](https://docs.rs/stream-download/latest/stream_download/source/trait.SourceStream.html) trait (enabled by default).
- `reqwest` - enables streaming content over http using [reqwest](https://crates.io/crates/reqwest) (enabled by default).
- `reqwest-native-tls` - enables reqwest's `native-tls` feature. Also enables the `reqwest` feature.
- `reqwest-rustls` - enables reqwest's `rustls` feature. Also enables the `reqwest` feature.
- `open-dal` - adds a `SourceStream` implementation that uses [Apache OpenDAL](https://crates.io/crates/opendal) as the backend.
- `temp-storage` - adds a temporary file-based storage backend (enabled by default).

One of `reqwest-native-tls` or `reqwest-rustls` is required if you wish to use https streams.

## Usage

```rust,no_run
use std::error::Error;
use std::io::Read;
use std::io;
use std::result::Result;

use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut reader = StreamDownload::new_http(
        "https://some-cool-url.com/some-file.mp3".parse()?,
        TempStorageProvider::new(),
        Settings::default(),
    )
    .await?;

    tokio::task::spawn_blocking(move || {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        Ok::<_, io::Error>(())
    }).await??;
    
    Ok(())
}

```

## Examples

See [examples](https://github.com/aschey/stream-download-rs/tree/main/examples).

## Transports

Transports implement the [`SourceStream`](https://docs.rs/stream-download/latest/stream_download/source/trait.SourceStream.html) trait. Two types of transports are provided out of the box - [`http`](https://docs.rs/stream-download/latest/stream_download/http) for typical HTTP-based sources and [`open_dal`](https://docs.rs/stream-download/latest/stream_download/open_dal) which is more complex, but supports a large variety of services.

Only `http` is enabled by default.
You can provide a custom transport by implementing `SourceStream` yourself.

## Streams with Unknown Length

Resources such as standalone songs or videos have a finite length that we use to support certain seeking functionality.
Infinite streams or those that otherwise don't have a known length are still supported, but attempting to seek from the end of the stream will return an error.
This may cause issues with certain audio or video libraries that attempt to perform such seek operations.
If it's necessary to explicitly check for an infinite stream, you can check the stream's content length ahead of time.

```rust,no_run
use std::error::Error;
use std::io::Read;
use std::io;
use std::result::Result;

use stream_download::http::HttpStream;
use stream_download::http::reqwest::Client;
use stream_download::source::SourceStream;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let stream =
        HttpStream::<Client>::create("https://some-cool-url.com/some-stream".parse()?).await?;
    let content_length = stream.content_length();
    let is_infinite = content_length.is_none();
    println!("Infinite stream = {is_infinite}");

    let mut reader =
        StreamDownload::from_stream(stream, TempStorageProvider::default(), Settings::default())
            .await?;

    tokio::task::spawn_blocking(move || {
        let mut buf = [0; 256];
        reader.read_exact(&mut buf)?;
        Ok::<_, io::Error>(())
    }).await??;
 
    Ok(())
}
```

### Icecast/Shoutcast Streams

If you're using this library to handle Icecast streams or one if its derivatives, check out the [icy-metadata](https://crates.io/crates/icy-metadata) crate.
There are examples for how to use it with stream-download [in the repo](https://github.com/aschey/icy-metadata/tree/main/examples).

## Storage

The [storage](https://docs.rs/stream-download/latest/stream_download/storage/index.html) module provides ways to customize how the stream is cached locally.
Pre-configured implementations are available for memory and temporary file-based storage.
Typically you'll want to use temporary file-based storage to prevent using too much memory, but memory-based storage may be preferable if you know the stream size is small or you need to run your application on a read-only filesystem.

```rust,no_run
use std::error::Error;
use std::io::Read;
use std::result::Result;

use stream_download::storage::memory::MemoryStorageProvider;
use stream_download::{Settings, StreamDownload};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut reader = StreamDownload::new_http(
        "https://some-cool-url.com/some-file.mp3".parse()?,
        // buffer will be stored in memory instead of on disk
        MemoryStorageProvider,
        Settings::default(),
    )
    .await?;

    Ok(())
}
```

### Bounded Storage

When using infinite streams which don't need to support seeking, it usually isn't desirable to let the underlying cache grow indefinitely if the stream may be running for a while.
For these cases, you may want to use [bounded storage](https://docs.rs/stream-download/latest/stream_download/storage/bounded/index.html).
Bounded storage uses a circular buffer which will overwrite the oldest contents once it fills up.

```rust,no_run
use std::error::Error;
use std::io::Read;
use std::num::NonZeroUsize;
use std::result::Result;

use stream_download::storage::bounded::BoundedStorageProvider;
use stream_download::storage::memory::MemoryStorageProvider;
use stream_download::{Settings, StreamDownload};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut reader = StreamDownload::new_http(
        "https://some-cool-url.com/some-file.mp3".parse()?,
        // use bounded storage to keep the underlying size from growing indefinitely
        BoundedStorageProvider::new(
            // you can use any other kind of storage provider here
            MemoryStorageProvider,
            // be liberal with the buffer size, you need to make sure it holds enough space to
            // prevent any out-of-bounds reads
            NonZeroUsize::new(512 * 1024).unwrap(),
        ),
        Settings::default(),
    )
    .await?;

    Ok(())
}
```

### Adaptive Storage

When you need to support both finite and infinite streams, you may want to use [adaptive storage](https://docs.rs/stream-download/latest/stream_download/storage/adaptive/index.html).
This is a convenience wrapper that will use bounded storage when the stream has no content length and unbounded storage when the stream does return a content length.

```rust,no_run
use std::error::Error;
use std::io::Read;
use std::num::NonZeroUsize;
use std::result::Result;

use stream_download::storage::adaptive::AdaptiveStorageProvider;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut reader = StreamDownload::new_http(
        "https://some-cool-url.com/some-file.mp3".parse()?,
        // use adaptive storage to keep the underlying size from growing indefinitely
        // when the content type is not known
        AdaptiveStorageProvider::new(
            // you can use any other kind of storage provider here
            TempStorageProvider::default(),
            // be liberal with the buffer size, you need to make sure it holds enough space to
            // prevent any out-of-bounds reads
            NonZeroUsize::new(512 * 1024).unwrap(),
        ),
        Settings::default(),
    )
    .await?;

    Ok(())
}
```

## Authentication and other customization

It's possible to customize your HTTP requests if you need to perform authentication or change other settings.

See [client_options](https://github.com/aschey/stream-download-rs/blob/main/examples/client_options.rs) for customizing the HTTP client builder.

See [custom_client](https://github.com/aschey/stream-download-rs/blob/main/examples/custom_client.rs) for dynamically modifying each HTTP request.

## Supported Rust Versions

The MSRV is currently `1.75.0`.
