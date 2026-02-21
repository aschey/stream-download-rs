use std::env::args;
use std::error::Error;

use reqwest_retry::RetryTransientMiddleware;
use reqwest_retry::policies::ExponentialBackoff;
use rodio::{Decoder, DeviceSinkBuilder, Player};
use stream_download::source::DecodeError;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::default().add_directive("stream_download=trace".parse()?))
        .with_line_number(true)
        .with_file(true)
        .init();

    let url = args().nth(1).unwrap_or_else(|| {
        "http://www.hyperion-records.co.uk/audiotest/14 Clementi Piano Sonata in D major, Op 25 No \
         6 - Movement 2 Un poco andante.MP3"
            .to_string()
    });

    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);

    // Instead of adding middleware globally like we do here, you can also create the client
    // manually like in reqwest-middleware's docs: https://github.com/TrueLayer/reqwest-middleware.
    // See the `client_options` example for how to pass in a client instance.
    Settings::add_default_middleware(RetryTransientMiddleware::new_with_policy(retry_policy));

    let reader = match StreamDownload::new_http_with_middleware(
        url.parse()?,
        TempStorageProvider::new(),
        Settings::default(),
    )
    .await
    {
        Ok(reader) => reader,
        Err(e) => return Err(e.decode_error().await)?,
    };

    let handle = tokio::task::spawn_blocking(move || {
        let sink = DeviceSinkBuilder::open_default_sink()?;
        let player = Player::connect_new(sink.mixer());
        player.append(Decoder::new(reader)?);
        player.sleep_until_end();
        Ok::<_, Box<dyn Error + Send + Sync>>(())
    });
    handle.await??;
    Ok(())
}
