use std::net::SocketAddr;
use std::sync::OnceLock;

use axum::Router;
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
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();

    SERVER_ADDR.get_or_init(|| listener.local_addr().unwrap());
    let service = ServeDir::new("./assets");
    let router = Router::new().nest_service("/", service);

    rt.spawn(async move {
        let listener = tokio::net::TcpListener::from_std(listener).unwrap();
        axum::serve(listener, router).await.unwrap();
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
