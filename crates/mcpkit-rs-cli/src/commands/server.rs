use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;

#[derive(Args)]
pub struct ServerArgs {
    /// Path to config.yaml
    #[arg(short, long)]
    config: PathBuf,

    /// Load from cached bundle
    #[arg(long)]
    from_bundle: Option<String>,

    /// Enable debug output
    #[arg(short, long)]
    debug: bool,

    /// Transport type (stdio, http, websocket)
    #[arg(short, long, default_value = "stdio")]
    transport: String,

    /// Bind address for HTTP/WebSocket
    #[arg(short, long, default_value = "127.0.0.1:3000")]
    bind: String,
}

pub async fn execute(args: ServerArgs) -> Result<()> {
    println!("{}", "🚀 Starting MCP server...".blue().bold());

    let config = if let Some(bundle_uri) = args.from_bundle {
        println!("  Loading from bundle: {}", bundle_uri.yellow());

        let cache =
            mcpkit_rs::bundle::BundleCache::new(mcpkit_rs::bundle::BundleCache::default_dir())?;
        let bundle = cache
            .get(&bundle_uri)
            .with_context(|| format!("Bundle not found in cache: {}", bundle_uri))?;

        serde_yaml::from_slice::<mcpkit_rs_config::Config>(&bundle.config)?
    } else {
        println!("  Config: {}", args.config.display());

        let config_str = std::fs::read_to_string(&args.config)
            .with_context(|| format!("Failed to read config: {}", args.config.display()))?;
        serde_yaml::from_str::<mcpkit_rs_config::Config>(&config_str)?
    };
    let transport = if args.transport != "stdio" {
        args.transport.clone()
    } else {
        match config.transport.transport_type {
            mcpkit_rs_config::TransportType::Stdio => "stdio",
            mcpkit_rs_config::TransportType::Http => "http",
            mcpkit_rs_config::TransportType::WebSocket => "websocket",
            mcpkit_rs_config::TransportType::Grpc => "grpc",
        }
        .to_string()
    };

    println!("  Transport: {}", transport.green());
    println!(
        "  Server: {} v{}",
        config.server.name, config.server.version
    );

    if let Some(desc) = &config.server.description {
        println!("  Description: {}", desc);
    }

    if let Some(mcp) = &config.mcp.capabilities {
        print!("  Capabilities:");
        if mcp.has_tools() {
            print!(" {}", "tools".cyan());
        }
        if mcp.has_prompts() {
            print!(" {}", "prompts".cyan());
        }
        if mcp.has_resources() {
            print!(" {}", "resources".cyan());
        }
        if mcp.has_logging() {
            print!(" {}", "logging".cyan());
        }
        println!();
    }

    if let Some(tools) = &config.mcp.tools {
        println!("  Tools: {} registered", tools.len());
        if args.debug {
            for tool in tools {
                println!("    • {}: {}", tool.name, tool.description);
            }
        }
    }

    println!();
    println!("{}", "Server is running. Press Ctrl+C to stop.".green());

    // TODO: Actually start the server based on config
    // This would integrate with mcpkit_rs service module

    tokio::signal::ctrl_c().await?;

    println!("\n{}", "👋 Server stopped".yellow());
    Ok(())
}
