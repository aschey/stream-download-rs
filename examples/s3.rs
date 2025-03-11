use std::error::Error;

use aws_sdk_s3 as s3;
use aws_sdk_s3::config::{BehaviorVersion, Region};
use aws_sdk_s3::primitives::ByteStream;
use opendal::layers::LoggingLayer;
use opendal::{Operator, services};
use stream_download::open_dal::OpenDalStreamParams;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};
use testcontainers_modules::localstack::LocalStack;
use testcontainers_modules::testcontainers::core::logs::LogFrame;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};
use tracing::info;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

static REGION: &str = "us-east-1";
static ACCESS_KEY_ID: &str = "test";
static SECRET_ACCESS_KEY: &str = "test";
static S3_KEY: &str = "music.mp3";
static S3_BUCKET: &str = "demo-bucket";

// Note: you will need Docker installed and running for this example to work

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::default().add_directive(LevelFilter::INFO.into()))
        .with_line_number(true)
        .with_file(true)
        .init();

    let request = LocalStack::default()
        .with_env_var("SERVICES", "s3")
        .with_log_consumer(|frame: &LogFrame| {
            let mut msg = std::str::from_utf8(frame.bytes()).unwrap();
            if msg.ends_with("\n") {
                msg = &msg[..msg.len() - 1];
            }
            info!("{msg}");
        });

    info!("starting container (may take a minute if we need to pull the image)...");
    let container = request.start().await?;
    info!("container started");

    let endpoint_url = setup_localstack_s3(&container).await?;

    let builder = services::S3::default()
        .region(REGION)
        .endpoint(&endpoint_url)
        .access_key_id(ACCESS_KEY_ID)
        .secret_access_key(SECRET_ACCESS_KEY)
        .bucket(S3_BUCKET);

    let operator = Operator::new(builder)?
        .layer(LoggingLayer::default())
        .finish();

    let reader = StreamDownload::new_open_dal(
        OpenDalStreamParams::new(operator, S3_KEY),
        TempStorageProvider::new(),
        Settings::default(),
    )
    .await?;

    tokio::task::spawn_blocking(move || {
        let (_stream, handle) = rodio::OutputStream::try_default()?;
        let sink = rodio::Sink::try_new(&handle)?;
        sink.append(rodio::Decoder::new(reader)?);
        sink.sleep_until_end();

        Ok::<_, Box<dyn Error + Send + Sync>>(())
    })
    .await??;

    Ok(())
}

async fn setup_localstack_s3(
    container: &ContainerAsync<LocalStack>,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let host_ip = container.get_host().await?;
    let host_port = container.get_host_port_ipv4(4566).await?;

    let endpoint_url = format!("http://{host_ip}:{host_port}");
    info!("localstack running on {endpoint_url}");

    let dummy_credentials =
        s3::config::Credentials::new(ACCESS_KEY_ID, SECRET_ACCESS_KEY, None, None, "test");

    let s3_config = aws_sdk_s3::config::Builder::default()
        .behavior_version(BehaviorVersion::v2025_01_17())
        .region(Region::new(REGION))
        .credentials_provider(dummy_credentials)
        .endpoint_url(&endpoint_url)
        .force_path_style(true)
        .build();

    let client = s3::Client::from_conf(s3_config);

    info!("creating bucket...");
    client.create_bucket().bucket(S3_BUCKET).send().await?;
    info!("bucket created");

    let audio_file = "./assets/music.mp3";

    info!("copying audio file...");

    client
        .put_object()
        .bucket(S3_BUCKET)
        .key(S3_KEY)
        .body(ByteStream::from_path(audio_file).await?)
        .send()
        .await?;
    info!("audio file copied");

    Ok(endpoint_url)
}
