use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub(super) struct Progress {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Debug)]
struct Inner;

impl Progress {
    pub(crate) fn restart(path: &std::path::PathBuf) -> Progress {
        todo!()
    }

    pub(crate) fn start_or_continue(path: &std::path::PathBuf) -> Progress {
        todo!()
    }

    pub(crate) fn note_seek(&self, pos: std::io::SeekFrom) {
        todo!()
    }
}
