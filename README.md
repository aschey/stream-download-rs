# stream-download-rs

![license](https://img.shields.io/badge/License-MIT%20or%20Apache%202-green.svg)
[![CI](https://github.com/aschey/stream-download-rs/actions/workflows/test.yml/badge.svg)](https://github.com/aschey/stream-download-rs/actions/workflows/build.yml)
![codecov](https://codecov.io/gh/aschey/stream-download-rs/branch/main/graph/badge.svg?token=Wx7OgIb0qa)
![GitHub repo size](https://img.shields.io/github/repo-size/aschey/stream-download-rs)
![Lines of Code](https://aschey.tech/tokei/github/aschey/stream-download-rs)

[stream-download](https://github.com/aschey/stream-download-rs) is a library for streaming content from a remote location and using it as a [read](https://doc.rust-lang.org/stable/std/io/trait.Read.html) and [seek](https://doc.rust-lang.org/stable/std/io/trait.Seek.html)-able source.
The file is downloaded in the background and you can perform read or seek operations before the download is finished.
This is useful for media applications that need to stream large files that may take several seconds or minutes to finish downloading.

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

One of `http-native-tls` or `http-rustls` is required if you wish to use https streams.

## Usage

```rust
use stream_download::{Settings, StreamDownload};

fn main() {
    let mut reader = StreamDownload::new_http(
        "https://some-cool-url.com/some-file.mp3".parse()?,
        Settings::default(),
    )?;

    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
}
```

See [examples](https://github.com/aschey/stream-download-rs/tree/main/examples).
