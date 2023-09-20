use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::mem;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rangemap::RangeSet;

#[derive(Debug, Clone)]
pub(super) struct Progress {
    inner: Arc<Mutex<Inner>>,
}

// a "serialized form" of the RangeSet that can not
// get corrupted by partial writes if the program
// abrubtly crashes
//
// format, binary: <Start of downloaded section><End of downloaded section>
// both as u64 little endian. Therefore each section is 16 bytes long
const SECTION_LENGTH: usize = 2 * mem::size_of::<u64>();
#[derive(Debug)]
struct Inner {
    file: File,
}

impl Inner {
    fn downloaded(&mut self) -> io::Result<RangeSet<u64>> {
        let mut buf = Vec::new();
        self.file.read_to_end(&mut buf)?;
        let ranges = buf
            .chunks_exact(SECTION_LENGTH)
            .map(|section| section.split_at(SECTION_LENGTH / 2))
            .map(|(a, b)| {
                (
                    u64::from_le_bytes(a.try_into().unwrap()),
                    u64::from_le_bytes(b.try_into().unwrap()),
                )
            })
            .map(|(start, end)| Range { start, end })
            .collect();
        Ok(ranges)
    }
}

impl Progress {
    pub(crate) fn restart(path: &Path) -> io::Result<Progress> {
        Self::new(path, true)
    }

    pub(crate) fn start_or_continue(path: &Path) -> io::Result<Progress> {
        Self::new(path, false)
    }

    fn new(file_path: &Path, restart: bool) -> io::Result<Progress> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .truncate(restart)
            .open(progress_path(file_path))?;
        remove_incomplete_writes(&mut file)?;
        if restart {
            todo!("init file")
        }

        Ok(Progress {
            inner: Arc::new(Mutex::new(Inner { file })),
        })
    }

    pub(crate) fn downloaded(&self) -> io::Result<RangeSet<u64>> {
        self.inner.lock().unwrap().downloaded()
    }

    pub(crate) fn finish_section(&self, end: u64) -> io::Result<()> {
        todo!()
    }
}

fn progress_path(file_path: &Path) -> PathBuf {
    file_path.join(".progress")
}

fn remove_incomplete_writes(file: &mut File) -> io::Result<()> {
    let current_len = file.metadata()?.len();
    let without_incomplete = current_len - current_len % SECTION_LENGTH as u64;
    file.set_len(without_incomplete)?;
    Ok(())
}
