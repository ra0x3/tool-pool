use std::sync::Arc;

use anyhow::Result;
use fullstack::FullStackServer;
use mcpkit_rs::{PolicyEnabledServer, ServiceExt};
use mcpkit_rs_config::Config;
use mcpkit_rs_policy::CompiledPolicy;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Disable all logging for stdio transport to prevent interference
    std::env::set_var("RUST_LOG", "off");
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_max_level(tracing::Level::ERROR)
        .init();

    std::env::set_var("STDIO_MODE", "true");

    // Load stdio-specific config from file - panic if not found
    let config = Config::from_yaml_file("config.stdio.yaml")?;
    let compiled_policy = {
        let policy = config
            .policy
            .expect("Policy must be defined in config.stdio.yaml");
        Arc::new(CompiledPolicy::compile(&policy)?)
    };

    // Create base server using sync version like http.rs
    let base_server = FullStackServer::new_with_compiled_policy(compiled_policy.clone());
    let server = PolicyEnabledServer::with_compiled_policy(base_server, compiled_policy);

    use fullstack::wasi_io;
    match server.serve(wasi_io()).await {
        Ok(service) => {
            let _ = service.waiting().await;
        }
        Err(_) => {
            // Silently fail - no output to stdio
        }
    }

    Ok(())
}
