use anyhow::Result;
use fullstack::FullStackServer;
use mcpkit_rs::ServiceExt;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let server = FullStackServer::new().await;
    use fullstack::wasi_io;
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
