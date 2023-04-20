use stream_download::StreamDownload;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();
    let (_stream, handle) = rodio::OutputStream::try_default().unwrap();
    let sink = rodio::Sink::try_new(&handle).unwrap();

    let reader = StreamDownload::new_http(
        "https://dl.espressif.com/dl/audio/ff-16b-2c-44100hz.flac"
            .parse()
            .unwrap(),
    );

    sink.append(rodio::Decoder::new(reader).unwrap());

    sink.sleep_until_end();
}
