use stream_download::{source::Settings, StreamDownload};
use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::default().add_directive("stream_download=trace".parse().unwrap()),
        )
        .with_line_number(true)
        .with_file(true)
        .init();
    let (_stream, handle) = rodio::OutputStream::try_default().unwrap();
    let sink = rodio::Sink::try_new(&handle).unwrap();

    let reader = StreamDownload::new_http(
        "http://www.hyperion-records.co.uk/audiotest/14 Clementi Piano Sonata in D major, Op 25 No 6 - Movement 2 Un poco andante.MP3"
            .parse()
            .unwrap(),
        Settings::default(),
    )
    .unwrap();

    sink.append(rodio::Decoder::new(reader).unwrap());

    sink.sleep_until_end();
}
