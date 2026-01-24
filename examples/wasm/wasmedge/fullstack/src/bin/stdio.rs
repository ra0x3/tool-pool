use anyhow::Result;
use fullstack::FullStackServer;
use mcpkit_rs::{PolicyEnabledServer, ServiceExt};
use mcpkit_rs_policy::Policy;

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

    // Create base server
    let base_server = FullStackServer::new().await;

    // Create a simple policy that only allows specific tools
    let policy_yaml = r#"
version: "1.0"
tools:
  allow:
    - test_connection
    - fetch_todos
    - create_todo
    - update_todo
    - delete_todo
    - batch_process
    - search_todos
    - db_stats
    - read_wal
  deny: []
"#;

    let policy = Policy::from_yaml(policy_yaml).expect("Failed to parse policy");
    let server = PolicyEnabledServer::with_policy(base_server, policy)
        .expect("Failed to create policy-enabled server");

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
