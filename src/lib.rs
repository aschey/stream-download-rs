use source::{Source, SourceHandle, SourceStream};
use std::{
    io::{self, BufReader, Read, Seek, SeekFrom},
    thread,
};
use tempfile::NamedTempFile;
use tracing::info;

#[cfg(feature = "http")]
pub mod http;
pub mod source;

pub struct StreamDownload {
    output_reader: BufReader<NamedTempFile>,
    handle: SourceHandle,
}

impl StreamDownload {
    #[cfg(feature = "http")]
    pub fn new_http(url: reqwest::Url) -> Self {
        Self::new::<http::HttpStream>(url)
    }

    pub fn new<S: SourceStream>(url: S::Url) -> Self {
        let tempfile = tempfile::Builder::new().tempfile().unwrap();
        let source = Source::<S>::create(url, tempfile.reopen().unwrap());
        let handle = source.source_handle();

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                source.download().await;
            });
        } else {
            thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();

                rt.block_on(async move {
                    source.download().await;
                });
            });
        };

        Self {
            output_reader: BufReader::new(tempfile),
            handle,
        }
    }
}

impl Read for StreamDownload {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let position = self.handle.position();
        let requested_position = position + buf.len() as u64;

        if let Some(closest_set) = self.handle.downloaded().get(&position) {
            if closest_set.end >= requested_position {
                return self.output_reader.read(buf);
            }
        }
        self.handle.request_position(requested_position);

        // info!(
        //     "read: current position {position} requested position {:?}. waiting",
        //     requested_position
        // );
        self.handle.wait_for_requested_position();

        // info!("reached requested position {requested_position}");
        self.output_reader.read(buf)
    }
}

impl Seek for StreamDownload {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        // let position = self.handle.position();

        let seek_pos = match pos {
            SeekFrom::Start(pos) => pos,
            SeekFrom::End(pos) => (self.handle.content_length().unwrap() as i64 + pos) as u64,
            SeekFrom::Current(pos) => (self.handle.position() as i64 + pos) as u64,
        };

        if let Some(closest_set) = self.handle.downloaded().get(&seek_pos) {
            if closest_set.end >= seek_pos {
                return self.output_reader.seek(pos);
            }
        }

        self.handle.request_position(seek_pos);
        // info!(
        //     "seek: current position {position} requested position {:?}. waiting",
        //     seek_pos
        // );
        self.handle.seek(seek_pos);
        self.handle.wait_for_requested_position();

        // info!("reached seek position");
        self.output_reader.seek(pos)
    }
}
