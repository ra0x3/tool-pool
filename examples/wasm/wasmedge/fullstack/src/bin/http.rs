use std::{env, sync::Arc, time::Duration};

use anyhow::Result;
use fullstack::FullStackServer;
use mcpkit_rs::{
    transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    },
    PolicyEnabledServer,
};
use mcpkit_rs_config::Config;
use mcpkit_rs_policy::CompiledPolicy;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug,rmcp=info".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    // Get bind address from environment or use default
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let bind_address = format!("{}:{}", host, port);

    eprintln!("=== Full-Stack HTTP Server (WasmEdge) ===");
    eprintln!("HOST env var: {:?}", env::var("HOST"));
    eprintln!("PORT env var: {:?}", env::var("PORT"));
    eprintln!("Starting HTTP server on: {}", bind_address);
    eprintln!("Real PostgreSQL & HTTP connections enabled");
    eprintln!();

    let ct = tokio_util::sync::CancellationToken::new();

    // Load HTTP-specific config from file - panic if not found
    let config = Config::from_yaml_file("config.http.yaml")?;
    let compiled_policy = {
        let policy = config
            .policy
            .expect("Policy must be defined in config.http.yaml");
        Arc::new(CompiledPolicy::compile(&policy)?)
    };

    let service = StreamableHttpService::new(
        {
            let compiled_policy = compiled_policy.clone();
            move || {
                // Create a new server instance for each session
                // The database connection will be attempted on first use
                let base_server =
                    FullStackServer::new_with_compiled_policy(compiled_policy.clone());
                let server =
                    PolicyEnabledServer::with_compiled_policy(base_server, compiled_policy.clone());
                Ok(server)
            }
        },
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig {
            cancellation_token: ct.child_token(),
            ..Default::default()
        },
    );

    // Configure CORS - must be permissive for Inspector
    // Use Any to allow all origins for development
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .expose_headers(Any)
        .allow_credentials(false)  // Set to false when using Any origin
        .max_age(Duration::from_secs(3600));

    let router = axum::Router::new()
        .nest_service("/mcp", service)
        .layer(cors);
    let tcp_listener = tokio::net::TcpListener::bind(&bind_address).await?;

    eprintln!("Server is listening at http://{}/mcp", bind_address);
    eprintln!("Note: Graceful shutdown not available in WASM - server will run indefinitely");

    axum::serve(tcp_listener, router).await?;

    Ok(())
}
