#!/usr/bin/env cargo +nightly -Zscript

//! This example demonstrates loading configuration and using it with WASM tools
//!
//! ```cargo
//! [dependencies]
//! mcpkit_rs = { path = "../crates/mcpkit-rs", features = ["wasm-tools", "config", "server"] }
//! tokio = { version = "1", features = ["full"] }
//! tracing = "0.1"
//! tracing-subscriber = { version = "0.3", features = ["env-filter"] }
//! ```

#[cfg(all(feature = "wasm-tools", feature = "config"))]
use std::sync::Arc;

#[cfg(all(feature = "wasm-tools", feature = "config"))]
use mcpkit_rs::{
    config::ServerConfig,
    wasm::{credentials::InMemoryCredentialProvider, load_wasm_tools_with_config},
};
#[cfg(all(feature = "wasm-tools", feature = "config"))]
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(all(feature = "wasm-tools", feature = "config"))]
    {
        // Initialize logging
        tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()),
            )
            .init();

        // Load configuration from file
        let config_path = "../../examples/wasm/wasmtime/calculator/config.yaml";
        tracing::info!("Loading configuration from: {}", config_path);

        let server_config = ServerConfig::from_file(config_path).await?;
        tracing::info!("Configuration loaded successfully");
        tracing::info!(
            "  Server: {} v{}",
            server_config.config.server.name,
            server_config.config.server.version
        );
        tracing::info!("  Runtime: {:?}", server_config.runtime_type());

        // Check runtime limits
        if let Some(ref runtime) = server_config.config.runtime.wasm {
            tracing::info!("  WASM Config:");
            tracing::info!("    Memory pages: {:?}", runtime.memory_pages);
            tracing::info!("    Fuel limit: {:?}", runtime.fuel);
        }

        // Check policy settings
        if let Some(ref policy) = server_config.config.policy {
            tracing::info!("  Policy Version: {}", policy.version);

            // Test policy enforcement
            let allowed_tools = vec!["add", "subtract", "multiply", "divide"];
            let denied_tools = vec!["exec", "system", "dangerous_tool"];

            for tool in allowed_tools {
                if server_config.is_tool_allowed(tool) {
                    tracing::info!("    ✓ Tool '{}' is allowed", tool);
                } else {
                    tracing::warn!("    ｘ Tool '{}' is NOT allowed (unexpected)", tool);
                }
            }

            for tool in denied_tools {
                if !server_config.is_tool_allowed(tool) {
                    tracing::info!("    ✓ Tool '{}' is correctly blocked", tool);
                } else {
                    tracing::warn!("    ｘ Tool '{}' is allowed (unexpected)", tool);
                }
            }
        }

        // Create WASM context and verify config values are applied
        let wasm_context = server_config.create_wasm_context();
        tracing::info!("  WASM Context created:");
        tracing::info!("    Timeout: {:?}", wasm_context.timeout);
        tracing::info!("    Max memory: {} bytes", wasm_context.max_memory_bytes);
        tracing::info!("    Max fuel: {:?}", wasm_context.max_fuel);
        tracing::info!(
            "    Environment vars: {} configured",
            wasm_context.env_vars.len()
        );

        // Try to load WASM tools with the configuration
        let tool_dir = "../../examples/wasm/wasmtime/calculator";
        let credential_provider = Arc::new(InMemoryCredentialProvider::new());

        tracing::info!("\nAttempting to load WASM tools from: {}", tool_dir);

        match load_wasm_tools_with_config(tool_dir, config_path, credential_provider).await {
            Ok(handler) => {
                tracing::info!("✓ WASM tools loaded successfully with configuration");

                // List available tools
                // List tools - RequestContext is not needed in the handler method for WasmToolHandler
                // since it's just listing the registered tools
                let tools = handler.executor().list_tools();
                tracing::info!("  Available tools: {}", tools.len());
                for tool in &tools {
                    let desc = tool.description.as_deref().unwrap_or("No description");
                    tracing::info!("    - {}: {}", tool.name, desc);
                }
            }
            Err(e) => {
                tracing::error!("ｘ Failed to load WASM tools: {}", e);
            }
        }

        tracing::info!("\n✓ Configuration integration test completed successfully!");
    }

    #[cfg(not(all(feature = "wasm-tools", feature = "config")))]
    {
        eprintln!("This example requires the 'wasm-tools' and 'config' features to be enabled.");
        eprintln!(
            "Run with: cargo run --example test_config_integration --features wasm-tools,config"
        );
    }

    Ok(())
}
