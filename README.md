# stream-download-rs

![license](https://img.shields.io/badge/License-MIT%20or%20Apache%202-green.svg)
[![CI](https://github.com/aschey/stream-download-rs/actions/workflows/test.yml/badge.svg)](https://github.com/aschey/stream-download-rs/actions/workflows/build.yml)
![codecov](https://codecov.io/gh/aschey/stream-download-rs/branch/main/graph/badge.svg?token=Wx7OgIb0qa)
![GitHub repo size](https://img.shields.io/github/repo-size/aschey/stream-download-rs)
![Lines of Code](https://aschey.tech/tokei/github/aschey/stream-download-rs)

[stream-download](https://github.com/aschey/stream-download-rs) is a library for streaming content from a remote location to a local file-backed cache and using it as a [read](https://doc.rust-lang.org/stable/std/io/trait.Read.html) and [seek](https://doc.rust-lang.org/stable/std/io/trait.Seek.html)-able source.
The requested content is downloaded in the background and read or seek operations are allowed before the download is finished. Seek operations may cause the stream to be restarted from the requested position if the download is still in progress.
This is useful for media applications that need to stream large files that may take a long time to download.

HTTP is the only transport supplied by this library, but you can use a custom transport by implementing the `SourceStream` trait.

We use [Tokio](https://tokio.rs) to download content using an asynchronous background task. If this library is used outside of a Tokio runtime, a single-threaded runtime will be started on a separate thread for you.

## Installation

```sh
cargo add stream-download
```

## Features

- `http` - adds an HTTP-based implementation of the `SourceStream` trait, but still requires an external HTTP client (enabled by default).
- `reqwest` - enables streaming content over http using [reqwest](https://github.com/seanmonstar/reqwest) (enabled by default).
- `reqwest-native-tls` - enables reqwest's `native-tls` feature. Also enables the `reqwest` feature.
- `reqwest-rustls` - enables reqwest's `rustls` feature. Also enables the `reqwest` feature.

One of `reqwest-native-tls` or `reqwest-rustls` is required if you wish to use https streams.

## Usage

```rust,no_run
use stream_download::{Settings, StreamDownload};
use std::{io::Read, result::Result, error::Error};

fn main() -> Result<(), Box<dyn Error>> {
    let mut reader = StreamDownload::new_http(
        "https://some-cool-url.com/some-file.mp3".parse()?,
        Settings::default(),
    )?;

    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
    Ok(())
}
```

See [examples](https://github.com/aschey/stream-download-rs/tree/main/examples).

## Streams with Unknown Length

Resources such as standalone songs or videos have a known length that we use to support certain seeking functionality.
Infinite streams or those that otherwise don't have a known length are still supported, but attempting to seek from the end of the stream will return an error.
This may cause issues with certain audio or video libraries that attempt to perform such seek operations.
If it's necessary to explicitly check for an infinite stream, you can check the stream's content length ahead of time. 

```rust,no_run
use stream_download::{Settings, StreamDownload, http::HttpStream, source::SourceStream};
use std::{io::Read, result::Result, error::Error};
use reqwest::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let stream = HttpStream::new(Client::new(), "https://some-cool-url.com/some-stream".parse()?).await?;
    let content_length = stream.content_length();
    let is_infinite = content_length.is_none();
    println!("Infinite stream = {is_infinite}");

    let mut reader = StreamDownload::from_stream(stream, Settings::default())?;

    let mut buf = [0; 256];
    reader.read_exact(&mut buf)?;
    Ok(())
}
```

## Supported Rust Versions

The MSRV is currently `1.63.0`.
