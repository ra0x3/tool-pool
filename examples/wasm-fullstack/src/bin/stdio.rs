use anyhow::Result;
use rmcp::ServiceExt;
use wasm_fullstack::FullStackServer;

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

    let server = FullStackServer::new().await;
    use wasm_fullstack::wasi_io;
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
