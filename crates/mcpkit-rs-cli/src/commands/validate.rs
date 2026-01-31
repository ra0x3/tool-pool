use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use mcpkit_rs_config::Config;

#[derive(Args)]
pub struct ValidateArgs {
    /// Path to config.yaml file
    config: PathBuf,

    /// Show detailed validation output
    #[arg(short, long)]
    verbose: bool,

    /// Validate distribution configuration
    #[arg(long)]
    check_distribution: bool,
}

pub async fn execute(args: ValidateArgs) -> Result<()> {
    println!("{}", "🔍 Validating configuration...".blue().bold());
    println!("  File: {}", args.config.display());

    let config_str = std::fs::read_to_string(&args.config)
        .with_context(|| format!("Failed to read config: {}", args.config.display()))?;

    let _yaml: serde_yaml::Value =
        serde_yaml::from_str(&config_str).context("Invalid YAML syntax")?;

    println!("  {} YAML syntax", "✓".green());

    let config: Config =
        serde_yaml::from_str(&config_str).context("Invalid configuration structure")?;

    println!("  {} Configuration structure", "✓".green());

    config
        .validate()
        .context("Configuration validation failed")?;

    println!("  {} Validation passed", "✓".green());

    if args.verbose {
        println!("\n{}", "Configuration Details:".cyan().bold());
        println!("  Version: {}", config.version);
        println!(
            "  Server: {} v{}",
            config.server.name, config.server.version
        );
        println!("  Transport: {:?}", config.transport.transport_type);
        println!("  Runtime: {:?}", config.runtime.runtime_type);
        println!("  MCP Version: {}", config.mcp.protocol_version);

        if let Some(tools) = &config.mcp.tools {
            println!("  Tools: {}", tools.len());
            for tool in tools {
                println!("    • {}: {}", tool.name.green(), tool.description);
            }
        }

        if let Some(policy) = &config.policy {
            println!("  Policy: v{}", policy.version);
            if let Some(desc) = &policy.description {
                println!("    {}", desc);
            }
        }
    }

    if args.check_distribution {
        println!("\n{}", "📦 Distribution Configuration:".cyan().bold());

        if let Some(dist) = &config.distribution {
            println!("  Registry: {}", dist.registry);
            if let Some(version) = &dist.version {
                println!("  Version: {}", version);
            }
            println!("  Tags: {}", dist.tags.join(", "));

            if let Some(metadata) = &dist.metadata {
                if !metadata.authors.is_empty() {
                    println!("  Authors: {}", metadata.authors.join(", "));
                }
                if let Some(license) = &metadata.license {
                    println!("  License: {}", license);
                }
                if !metadata.keywords.is_empty() {
                    println!("  Keywords: {}", metadata.keywords.join(", "));
                }
            }

            if dist.auth.is_some() {
                println!("  {} Authentication configured", "✓".green());
            }

            println!("  {} Distribution config valid", "✓".green());
        } else {
            println!("  {} No distribution configuration found", "⚠".yellow());
        }
    }

    println!("\n{}", "✨ Configuration is valid!".green().bold());
    Ok(())
}
