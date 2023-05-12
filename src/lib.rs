use source::{Source, SourceHandle, SourceStream};
use std::{
    io::{self, BufReader, Read, Seek, SeekFrom},
    thread,
};
use tempfile::NamedTempFile;
use tracing::debug;

#[cfg(feature = "http")]
pub mod http;
pub mod source;

#[derive(Debug)]
pub struct StreamDownload {
    output_reader: BufReader<NamedTempFile>,
    handle: SourceHandle,
    read_position: u64,
}

impl StreamDownload {
    #[cfg(feature = "http")]
    pub fn new_http(url: reqwest::Url) -> Self {
        Self::new::<http::HttpStream>(url)
    }

    pub fn new<S: SourceStream>(url: S::Url) -> Self {
        let tempfile = tempfile::Builder::new().tempfile().unwrap();
        let source = Source::new(tempfile.reopen().unwrap());
        let handle = source.source_handle();

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let stream = S::create(url).await;
                source.download(stream).await;
            });
        } else {
            thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();

                rt.block_on(async move {
                    let stream = S::create(url).await;
                    source.download(stream).await;
                });
            });
        };

        Self {
            output_reader: BufReader::new(tempfile),
            read_position: 0,
            handle,
        }
    }

    pub fn from_stream<S: SourceStream>(stream: S) -> Self {
        let tempfile = tempfile::Builder::new().tempfile().unwrap();
        let source = Source::new(tempfile.reopen().unwrap());
        let handle = source.source_handle();

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                source.download(stream).await;
            });
        } else {
            thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();

                rt.block_on(async move {
                    source.download(stream).await;
                });
            });
        };

        Self {
            output_reader: BufReader::new(tempfile),
            handle,
            read_position: 0,
        }
    }
}

impl Read for StreamDownload {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        debug!("Read request buf len: {}", buf.len());

        let requested_position = self.read_position + buf.len() as u64;
        debug!(
            "read: current position: {} requested position: {requested_position}",
            self.read_position
        );

        if let Some(closest_set) = self.handle.downloaded().get(&self.read_position) {
            debug!("Already downloaded {closest_set:?}");
            if closest_set.end >= requested_position {
                let read_len = self.output_reader.read(buf);
                if let Ok(read_len) = read_len {
                    self.read_position += read_len as u64;
                }
                return read_len;
            }
        }
        self.handle.request_position(requested_position);

        debug!("waiting for position");
        self.handle.wait_for_requested_position();

        debug!("reached requested position {requested_position}");
        self.output_reader.read(buf)
    }
}

impl Seek for StreamDownload {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let seek_pos = match pos {
            SeekFrom::Start(pos) => pos,
            SeekFrom::End(pos) => {
                if let Some(length) = self.handle.content_length() {
                    (length as i64 + pos) as u64
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "Cannot seek from end when content length is unknown",
                    ));
                }
            }
            SeekFrom::Current(pos) => (self.read_position as i64 + pos) as u64,
        };

        if let Some(closest_set) = self.handle.downloaded().get(&seek_pos) {
            if closest_set.end >= seek_pos {
                let new_pos = self.output_reader.seek(pos);
                if let Ok(new_pos) = new_pos {
                    self.read_position = new_pos;
                }
            }
        }

        self.handle.request_position(seek_pos);
        debug!(
            "seek: current position {seek_pos} requested position {:?}. waiting",
            seek_pos
        );
        self.handle.seek(seek_pos);
        self.handle.wait_for_requested_position();

        debug!("reached seek position");
        self.output_reader.seek(pos)
    }
}
