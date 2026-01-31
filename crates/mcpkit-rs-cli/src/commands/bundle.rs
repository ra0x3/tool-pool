use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Subcommand;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use mcpkit_rs::bundle::{BundleCache, BundleClient};
use mcpkit_rs_config::Config;

#[derive(Subcommand)]
pub enum BundleCommands {
    /// Push a bundle to an OCI registry
    Push {
        /// Path to WASM module
        #[arg(short, long)]
        wasm: PathBuf,

        /// Path to config.yaml
        #[arg(short, long)]
        config: PathBuf,

        /// OCI registry URI (e.g., oci://ghcr.io/org/bundle:tag)
        #[arg(short, long)]
        uri: Option<String>,

        /// Skip digest verification
        #[arg(long)]
        no_verify: bool,
    },

    /// Pull a bundle from an OCI registry
    Pull {
        /// OCI registry URI
        uri: String,

        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Force overwrite if exists
        #[arg(short, long)]
        force: bool,
    },

    /// List cached bundles
    List {
        /// Show detailed information
        #[arg(short, long)]
        verbose: bool,
    },

    /// Clear bundle cache
    Cache {
        /// Clear all cached bundles
        #[arg(long)]
        clear: bool,

        /// Show cache statistics
        #[arg(long)]
        stats: bool,

        /// Verify cache integrity
        #[arg(long)]
        verify: bool,
    },
}

pub async fn execute(cmd: BundleCommands) -> Result<()> {
    match cmd {
        BundleCommands::Push {
            wasm,
            config,
            uri,
            no_verify,
        } => push_bundle(wasm, config, uri, no_verify).await,
        BundleCommands::Pull { uri, output, force } => pull_bundle(uri, output, force).await,
        BundleCommands::List { verbose } => list_bundles(verbose).await,
        BundleCommands::Cache {
            clear,
            stats,
            verify,
        } => manage_cache(clear, stats, verify).await,
    }
}

async fn push_bundle(
    wasm_path: PathBuf,
    config_path: PathBuf,
    uri: Option<String>,
    no_verify: bool,
) -> Result<()> {
    println!("{}", "📦 Pushing bundle...".blue().bold());

    let wasm = std::fs::read(&wasm_path)
        .with_context(|| format!("Failed to read WASM file: {}", wasm_path.display()))?;
    let config_bytes = std::fs::read(&config_path)
        .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

    let config: Config =
        serde_yaml::from_slice(&config_bytes).context("Failed to parse config.yaml")?;

    let uri = uri
        .or_else(|| {
            config.distribution.as_ref().map(|d| {
                let tag = d.tags.first().map(|t| t.as_str()).unwrap_or("latest");
                format!("oci://{}:{}", d.registry, tag)
            })
        })
        .context("No URI specified and no distribution config found")?;

    println!("  Target: {}", uri.yellow());

    let auth = config
        .distribution
        .as_ref()
        .and_then(|d| d.auth.as_ref())
        .cloned()
        .or_else(|| {
            let username = std::env::var("GITHUB_USER").ok();
            let token = std::env::var("GITHUB_TOKEN").ok();

            if username.is_some() || token.is_some() {
                Some(mcpkit_rs_config::RegistryAuth {
                    username,
                    password: token,
                    auth_file: None,
                    use_keychain: false,
                })
            } else {
                None
            }
        });

    let client = BundleClient::new();

    let progress = ProgressBar::new_spinner();
    progress.set_style(ProgressStyle::default_spinner().template("{spinner:.green} {msg}")?);
    progress.set_message("Uploading layers...");

    let digest = match client.push(&wasm, &config_bytes, &uri, auth.as_ref()).await {
        Ok(digest) => digest,
        Err(e) => {
            if e.to_string().contains("AuthenticationRequired") {
                eprintln!("{}", "❌ Authentication failed!".red().bold());
                eprintln!();
                eprintln!("For GitHub Container Registry:");
                eprintln!("  1. Set GITHUB_USER to your GitHub username");
                eprintln!(
                    "  2. Set GITHUB_TOKEN to a personal access token with 'write:packages' scope"
                );
                eprintln!();
                eprintln!("Example:");
                eprintln!("  export GITHUB_USER=yourname");
                eprintln!("  export GITHUB_TOKEN=ghp_xxxxxxxxxxxxxxxxxxxx");
                eprintln!();
                eprintln!("To create a token: https://github.com/settings/tokens/new");
                std::process::exit(1);
            } else if e.to_string().contains("environment variable")
                && e.to_string().contains("not set")
            {
                eprintln!("{}", "❌ Missing environment variable!".red().bold());
                eprintln!();
                eprintln!("{}", e);
                eprintln!();
                eprintln!("Please set the required environment variable and try again.");
                std::process::exit(1);
            }
            return Err(e.into());
        }
    };

    progress.finish_with_message("✓ Upload complete");

    if !no_verify {
        println!("  Digest: {}", digest.green());
    }

    println!("{}", "✨ Bundle pushed successfully!".green().bold());
    Ok(())
}

async fn pull_bundle(uri: String, output: Option<PathBuf>, force: bool) -> Result<()> {
    println!("{}", "📥 Pulling bundle...".blue().bold());
    println!("  Source: {}", uri.yellow());

    let cache_dir = BundleCache::default_dir();
    let cache = BundleCache::new(&cache_dir)?;

    let client = BundleClient::with_cache(cache);

    let auth = {
        let username = std::env::var("GITHUB_USER").ok();
        let token = std::env::var("GITHUB_TOKEN").ok();

        if username.is_some() || token.is_some() {
            Some(mcpkit_rs_config::RegistryAuth {
                username,
                password: token,
                auth_file: None,
                use_keychain: false,
            })
        } else {
            None
        }
    };

    let progress = ProgressBar::new_spinner();
    progress.set_style(ProgressStyle::default_spinner().template("{spinner:.green} {msg}")?);
    progress.set_message("Downloading bundle...");

    let bundle = match client.pull(&uri, auth.as_ref()).await {
        Ok(bundle) => bundle,
        Err(e) => {
            if e.to_string().contains("AuthenticationRequired") {
                eprintln!("{}", "❌ Authentication required!".red().bold());
                eprintln!();
                eprintln!("This registry requires authentication to pull bundles.");
                eprintln!();
                eprintln!("For GitHub Container Registry:");
                eprintln!("  export GITHUB_USER=yourname");
                eprintln!("  export GITHUB_TOKEN=ghp_xxxxxxxxxxxxxxxxxxxx");
                eprintln!();
                eprintln!("Note: For public repositories, you may only need read:packages scope");
                std::process::exit(1);
            }
            return Err(anyhow::anyhow!("Failed to pull bundle: {}", e));
        }
    };

    progress.finish_with_message("✓ Download complete");

    bundle.verify().context("Bundle verification failed")?;

    if let Some(output_dir) = output {
        if output_dir.exists() && !force {
            anyhow::bail!("Output directory exists. Use --force to overwrite");
        }

        std::fs::create_dir_all(&output_dir)?;
        bundle.save_to_directory(&output_dir)?;

        println!("  Saved to: {}", output_dir.display());
    }

    println!("{}", "✨ Bundle pulled successfully!".green().bold());
    Ok(())
}

async fn list_bundles(verbose: bool) -> Result<()> {
    let cache = BundleCache::new(BundleCache::default_dir())?;
    let bundles = cache.list()?;

    if bundles.is_empty() {
        println!("{}", "No cached bundles found".yellow());
        return Ok(());
    }

    println!(
        "{}",
        format!("📦 {} cached bundle(s):", bundles.len())
            .blue()
            .bold()
    );

    for uri in bundles {
        if verbose {
            if let Ok(bundle) = cache.get(&uri) {
                println!("\n  {}", uri.green());
                println!("    Registry: {}", bundle.metadata.registry);
                println!("    Version: {}", bundle.metadata.version);
                println!("    WASM size: {} bytes", bundle.wasm.len());
                println!("    Config size: {} bytes", bundle.config.len());
            }
        } else {
            println!("  • {}", uri);
        }
    }

    Ok(())
}

async fn manage_cache(clear: bool, stats: bool, verify: bool) -> Result<()> {
    let cache = BundleCache::new(BundleCache::default_dir())?;

    if clear {
        cache.clear()?;
        println!("{}", "✨ Cache cleared successfully!".green().bold());
    }

    if stats {
        let stats = cache.stats()?;
        println!("{}", "📊 Cache Statistics:".blue().bold());
        println!("  Location: {}", stats.cache_dir.display());
        println!("  Bundles: {}", stats.bundle_count);
        println!("  Total size: {}", stats.format_size());
    }

    if verify {
        println!("{}", "🔍 Verifying cache integrity...".blue().bold());
        let corrupted = cache.verify()?;

        if corrupted.is_empty() {
            println!("{}", "✓ All bundles verified successfully".green());
        } else {
            println!(
                "{}",
                format!("⚠ {} corrupted bundle(s) found:", corrupted.len()).yellow()
            );
            for uri in corrupted {
                println!("  • {}", uri.red());
            }
        }
    }

    Ok(())
}
