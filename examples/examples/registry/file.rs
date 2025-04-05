use std::fs::File;
use std::io::BufReader;

use async_trait::async_trait;
use stream_download::registry::{Input, RegistryEntry, Rule};
use tracing::info;

use crate::{Reader, Result};

pub struct FileResolver {
    rules: Vec<Rule>,
}

impl FileResolver {
    pub fn new() -> Self {
        Self {
            rules: vec![Rule::url_scheme("file"), Rule::any_string()],
        }
    }
}

#[async_trait]
impl RegistryEntry<Result<Reader>> for FileResolver {
    fn priority(&self) -> u32 {
        // Check for file URLs and paths third
        3
    }

    fn rules(&self) -> &[Rule] {
        &self.rules
    }

    async fn handler(&mut self, input: Input) -> Result<Reader> {
        info!("using file resolver");
        let path = input.source.to_string();
        let file = File::open(&path)?;

        Ok(Reader::File(BufReader::new(file)))
    }
}
