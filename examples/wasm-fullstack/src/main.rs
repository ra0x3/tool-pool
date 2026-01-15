use anyhow::Result;
use rmcp::ServiceExt;
use wasm_fullstack::FullStackServer;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let server = FullStackServer::new().await;
    use wasm_fullstack::wasi_io;
    match server.serve(wasi_io()).await {
        Ok(service) => {
            tracing::info!("Full-Stack Server running");
            if let Err(e) = service.waiting().await {
                tracing::error!("Server error: {:?}", e);
            }
        }
        Err(e) => {
            tracing::error!("Failed to start server: {:?}", e);
        }
    }

    Ok(())
}
