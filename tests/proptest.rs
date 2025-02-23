use std::io::Read;
use std::num::NonZeroUsize;
use std::{fs, io};

use proptest::prelude::*;
use setup::{SERVER_RT, server_addr};
use stream_download::storage::bounded::BoundedStorageProvider;
use stream_download::storage::memory::MemoryStorageProvider;
use stream_download::{Settings, StreamDownload};

mod setup;

#[derive(Debug)]
struct StreamParams {
    read_len: usize,
    bounded_size: usize,
    prefetch_bytes: u64,
}

prop_compose! {
    fn input_sizes()
        ((read_len, bounded_size) in get_bounded_size())
        (
            read_len in Just(read_len),
            bounded_size in Just(bounded_size),
            prefetch_bytes in 0..bounded_size*2) -> StreamParams {
            StreamParams { read_len, bounded_size, prefetch_bytes: prefetch_bytes as u64 }
    }
}

prop_compose! {
    fn get_read_len()(read_len in 16*1024..64*1024usize) -> usize {
        read_len
    }
}

prop_compose! {
    fn get_bounded_size()
        (read_len in get_read_len())
        (read_len in Just(read_len), bounded_size in 300*1024..400*1024usize) -> (usize, usize) {
        (read_len, bounded_size)
    }
}

proptest! {
    #[test]
    fn proptest(StreamParams { read_len, bounded_size, prefetch_bytes } in input_sizes()) {
        let buf = SERVER_RT.block_on(async move {
            let mut reader = StreamDownload::new_http(
                format!("http://{}/music.mp3", server_addr())
                    .parse()
                    .unwrap(),
                BoundedStorageProvider::new(MemoryStorageProvider,
                    NonZeroUsize::new(bounded_size).unwrap()),
                Settings::default().prefetch_bytes(prefetch_bytes),
            )
            .await
            .unwrap();

            tokio::task::spawn_blocking(move || {
                let mut buf = Vec::<u8>::new();
                let mut temp_buf = vec![0; read_len];

                loop {
                    let read_len = reader.read(&mut temp_buf)?;
                    buf.extend(&temp_buf[..read_len]);
                    if read_len == 0 {
                        break;
                    }
                }
                Ok::<_,io::Error>(buf)
            }).await
        }).unwrap().unwrap();

        let file_buf = get_file_buf();
        assert_eq!(file_buf.len(), buf.len());
        compare(get_file_buf(), buf);
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
