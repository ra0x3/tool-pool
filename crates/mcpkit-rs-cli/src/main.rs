use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod commands;

use commands::{bundle, server, validate};

#[derive(Parser)]
#[command(name = "mcpk")]
#[command(version)]
#[command(about = "MCPKit-RS CLI - WebAssembly MCP toolkit", long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Set log level (trace, debug, info, warn, error)
    #[arg(short, long, env = "MCPKIT_LOG_LEVEL", default_value = "info")]
    log_level: String,

    /// Disable colored output
    #[arg(long, env = "NO_COLOR")]
    no_color: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage WASM bundles and distribution
    #[command(subcommand)]
    Bundle(bundle::BundleCommands),

    /// Run an MCP server
    Server(server::ServerArgs),

    /// Validate configuration files
    Validate(validate::ValidateArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&cli.log_level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    colored::control::set_override(!cli.no_color);

    match cli.command {
        Commands::Bundle(cmd) => bundle::execute(cmd).await,
        Commands::Server(args) => server::execute(args).await,
        Commands::Validate(args) => validate::execute(args).await,
    }
}
