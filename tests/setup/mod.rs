use std::net::SocketAddr;
use std::sync::OnceLock;

use ctor::ctor;
use tokio::runtime::Runtime;
use tower_http::services::ServeDir;
use tracing_subscriber::EnvFilter;

pub static SERVER_RT: OnceLock<Runtime> = OnceLock::new();
pub static SERVER_ADDR: OnceLock<SocketAddr> = OnceLock::new();

#[ctor]
fn setup() {
    setup_logger();

    let rt = SERVER_RT.get_or_init(|| Runtime::new().unwrap());
    let _guard = rt.enter();
    let service = ServeDir::new("./assets");

    let server = hyper::Server::try_bind(&"127.0.0.1:0".parse().unwrap())
        .unwrap()
        .serve(tower::make::Shared::new(service));
    SERVER_ADDR.get_or_init(|| server.local_addr());

    rt.spawn(async move {
        server.await.unwrap();
    });
}

fn setup_logger() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_line_number(true)
        .with_file(true)
        .with_test_writer()
        .init();
}
