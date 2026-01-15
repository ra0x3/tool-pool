use anyhow::Result;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use wasm_fullstack::FullStackServer;

const BIND_ADDRESS: &str = "127.0.0.1:8080";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug,rmcp=info".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    eprintln!("=== Full-Stack HTTP Server (WasmEdge) ===");
    eprintln!("Starting HTTP server on: {}", BIND_ADDRESS);
    eprintln!("Real PostgreSQL & HTTP connections enabled");
    eprintln!();

    let ct = tokio_util::sync::CancellationToken::new();

    let service = StreamableHttpService::new(
        || {
            // Create a new server instance for each session
            // The database connection will be attempted on first use
            let server = FullStackServer::new_sync();
            Ok(server)
        },
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig {
            cancellation_token: ct.child_token(),
            ..Default::default()
        },
    );

    let router = axum::Router::new().nest_service("/mcp", service);
    let tcp_listener = tokio::net::TcpListener::bind(BIND_ADDRESS).await?;

    eprintln!("Server is listening at http://{}/mcp", BIND_ADDRESS);
    eprintln!("Note: Graceful shutdown not available in WASM - server will run indefinitely");

    axum::serve(tcp_listener, router).await?;

    Ok(())
}
