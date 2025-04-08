use std::num::NonZeroUsize;

use async_trait::async_trait;
use stream_download::registry::{Input, RegistryEntry, Rule};
use stream_download::source::DecodeError;
use stream_download::storage::adaptive::AdaptiveStorageProvider;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};
use tracing::info;

use crate::{Reader, Result};

pub struct HttpResolver {
    rules: Vec<Rule>,
}

impl HttpResolver {
    pub fn new() -> Self {
        Self {
            rules: vec![Rule::any_http()],
        }
    }
}

#[async_trait]
impl RegistryEntry<Result<Reader>> for HttpResolver {
    fn priority(&self) -> u32 {
        // Check for generic URLs second
        2
    }

    fn rules(&self) -> &[Rule] {
        &self.rules
    }

    async fn handler(&mut self, input: Input) -> Result<Reader> {
        info!("using http resolver");
        let settings = Settings::default();
        let reader = match StreamDownload::new_http(
            input.source.into_url(),
            // use adaptive storage to keep the underlying size bounded when the stream has no
            // content length
            AdaptiveStorageProvider::new(
                TempStorageProvider::default(),
                // ensure we have enough buffer space to store the prefetch data
                NonZeroUsize::new((settings.get_prefetch_bytes() * 2) as usize).unwrap(),
            ),
            settings,
        )
        .await
        {
            Ok(reader) => reader,
            Err(e) => return Err(e.decode_error().await)?,
        };
        Ok(Reader::Stream(reader))
    }
}
