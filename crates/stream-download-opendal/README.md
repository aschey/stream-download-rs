# stream-download-opendal

[![crates.io](https://img.shields.io/crates/v/stream-download-opendal.svg?logo=rust)](https://crates.io/crates/stream-download-opendal)
[![docs.rs](https://img.shields.io/docsrs/stream-download-opendal?logo=rust)](https://docs.rs/stream-download-opendal)
[![Dependency Status](https://deps.rs/repo/github/aschey/stream-download-rs/status.svg?style=flat-square)](https://deps.rs/repo/github/aschey/stream-download-rs)
![license](https://img.shields.io/badge/License-MIT%20or%20Apache%202-green.svg)
[![CI](https://github.com/aschey/stream-download-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/aschey/stream-download-rs/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/aschey/stream-download-rs/branch/main/graph/badge.svg?token=Wx7OgIb0qa)](https://app.codecov.io/gh/aschey/stream-download-rs)
![GitHub repo size](https://img.shields.io/github/repo-size/aschey/stream-download-rs)
![Lines of Code](https://aschey.tech/tokei/github/aschey/stream-download-rs)

`stream-download-opendal` provides integration between
[`stream-download`](https://crates.io/crates/stream-download) and
[`opendal`](https://crates.io/crates/opendal).

`OpenDAL` is a data access layer that supports data retrieval from a variety of
storage services. The list of supported services is
[documented here](https://docs.rs/opendal/latest/opendal/services/index.html).

## Example using S3

```rust,no_run
 use std::error::Error;

 use opendal::{Operator, services};
 use stream_download::storage::temp::TempStorageProvider;
 use stream_download::{Settings, StreamDownload};
 use stream_download_opendal::{OpendalStream, OpendalStreamParams};

 #[tokio::main]
 async fn main() -> Result<(), Box<dyn Error>> {
     let mut builder = services::S3::default()
         .region("us-east-1")
         .access_key_id("test")
         .secret_access_key("test")
         .bucket("my-bucket");

     let operator = Operator::new(builder)?.finish();
     let stream = OpendalStream::new(OpendalStreamParams::new(operator, "some-object-key")).await?;

     Ok(())
 }
```
