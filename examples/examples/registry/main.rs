use std::env::args;
use std::error::Error;
use std::fs::File;
use std::io;

use file::FileResolver;
use http::HttpResolver;
use rodio::{Decoder, DeviceSinkBuilder, Player};
use stream_download::StreamDownload;
use stream_download::registry::Registry;
use stream_download::storage::adaptive::AdaptiveStorageProvider;
use stream_download::storage::temp::TempStorageProvider;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use yt_dlp::YtDlpResolver;

mod file;
mod http;
mod yt_dlp;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
type Downloader = StreamDownload<AdaptiveStorageProvider<TempStorageProvider, TempStorageProvider>>;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::default().add_directive(LevelFilter::INFO.into()))
        .with_line_number(true)
        .with_file(true)
        .init();

    let Some(url) = args().nth(1) else {
        Err("Usage: cargo run --example=registry -- <url or file path>")?
    };

    let mut registry = Registry::new()
        .entry(YtDlpResolver::new())
        .entry(HttpResolver::new())
        .entry(FileResolver::new());

    let reader = registry
        .find_match(&url)
        .await
        .ok_or_else(|| format!("don't know know to handle input: {url}"))??;

    let reader_handle = if let Reader::Stream(stream) = &reader {
        Some(stream.handle())
    } else {
        None
    };

    let handle = tokio::task::spawn_blocking(move || {
        let sink = DeviceSinkBuilder::open_default_sink()?;
        let player = Player::connect_new(sink.mixer());
        player.append(Decoder::new(reader)?);
        player.sleep_until_end();

        Ok::<_, Box<dyn Error + Send + Sync>>(())
    });
    handle.await??;

    if let Some(handle) = reader_handle {
        // Wait for the stream to terminate gracefully
        handle.wait_for_completion().await;
    }

    Ok(())
}

enum Reader {
    Stream(Downloader),
    File(io::BufReader<File>),
}

impl io::Read for Reader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Stream(r) => r.read(buf),
            Self::File(r) => r.read(buf),
        }
    }
}

impl io::Seek for Reader {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        match self {
            Self::Stream(r) => r.seek(pos),
            Self::File(r) => r.seek(pos),
        }
    }
}
