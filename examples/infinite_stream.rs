use std::error::Error;
use stream_download::{Settings, StreamDownload};
use tracing_subscriber::EnvFilter;

fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::default().add_directive("stream_download=trace".parse()?))
        .with_line_number(true)
        .with_file(true)
        .init();

    let (_stream, handle) = rodio::OutputStream::try_default()?;
    let sink = rodio::Sink::try_new(&handle)?;

    let reader = StreamDownload::new_http(
        "https://us2.internet-radio.com/proxy/megatoncafe?mp=/stream".parse()?,
        Settings::default(),
    )?;

    sink.append(rodio::Decoder::new(reader)?);
    sink.sleep_until_end();
    Ok(())
}
