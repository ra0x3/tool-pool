# mcpkit-rs Distribution System

## Table of Contents

- [Overview](#overview)
  - [Goals](#goals)
  - [Non-Goals](#non-goals)
- [Architecture](#architecture)
  - [Bundle Format](#bundle-format)
  - [URI Schemes](#uri-schemes)
  - [Storage Layout](#storage-layout)
- [Implementation Details](#implementation-details)
  - [OCI Registry Integration](#oci-registry-integration)
  - [Configuration Structure](#configuration-structure)
  - [Publishing Flow](#publishing-flow)
  - [Consumption Flow](#consumption-flow)
- [Comparison with Wassette](#comparison-with-wassette)
  - [Similarities](#similarities)
  - [Differences](#differences)
  - [Trade-offs](#trade-offs)
- [Security Considerations](#security-considerations)
  - [Digest Verification](#digest-verification)
  - [Registry Authentication](#registry-authentication)
  - [Trust Model](#trust-model)
- [Implementation Plan](#implementation-plan)
  - [Phase 1: Core Infrastructure](#phase-1-core-infrastructure)
  - [Phase 2: OCI Integration](#phase-2-oci-integration)
  - [Phase 3: CLI Commands](#phase-3-cli-commands)
  - [Phase 4: Testing](#phase-4-testing)
- [Testing Strategy](#testing-strategy)
  - [Unit Tests](#unit-tests)
  - [Integration Tests](#integration-tests)
  - [End-to-End Tests](#end-to-end-tests)

## Overview

The mcpkit-rs distribution system enables sharing and deployment of MCP WASM modules through OCI registries. It leverages existing container infrastructure (GitHub Container Registry, Docker Hub) to distribute WASM bundles without building new distribution infrastructure.

### Goals

1. **Minimal Overhead** - Reuse existing OCI infrastructure and tooling
2. **Single Configuration** - Use existing config.yaml with distribution section
3. **Simple Bundle Format** - WASM binary + config.yaml, nothing more
4. **Registry Agnostic** - Support any OCI-compliant registry
5. **Local Caching** - Efficient local storage with precompiled modules
6. **Secure by Default** - SHA256 digest verification on all artifacts

### Non-Goals

1. **New Registry Infrastructure** - We use existing OCI registries
2. **Complex Dependency Resolution** - Simple direct dependencies only
3. **Package Management** - Not trying to be npm/cargo for WASM
4. **Signature Verification** - May add Cosign support later but not v1
5. **P2P Distribution** - Registry-based only for simplicity

## Architecture

```
┌─────────────────────────────────────────────┐
│                User Machine                  │
├───────────────┬─────────────┬───────────────┤
│  config.yaml  │ module.wasm │  mcpkit CLI   │
└───────┬───────┴──────┬──────┴───────┬───────┘
        │              │              │
        │   Bundle     │     Push     │
        └──────────────┴──────────────▼
                              ┌────────────────┐
                              │  OCI Registry  │
                              │   (GitHub)     │
                              └────────┬───────┘
                                       │
                                  Pull │
                                       ▼
        ┌──────────────────────────────────────┐
        │          Consumer Machine            │
        ├──────────────┬───────────────────────┤
        │ ~/.mcpkit/   │   mcpkit server       │
        │   bundles/   │   --from-bundle       │
        └──────────────┴───────────────────────┘
```

### Bundle Format

A bundle consists of exactly two files:

1. **module.wasm** - The compiled WebAssembly module
2. **config.yaml** - Standard mcpkit config with distribution section

These are pushed as two layers in an OCI image:
- Layer 0: WASM binary (media type: `application/wasm`)
- Layer 1: Configuration (media type: `application/vnd.mcpkit.config+yaml`)

### URI Schemes

```
oci://ghcr.io/org/bundle:tag       # GitHub Container Registry
oci://docker.io/org/bundle:v1.0.0  # Docker Hub
file:///path/to/local/bundle.wasm  # Local filesystem
https://example.com/bundle.tar.gz  # Direct HTTP (future)
```

### Storage Layout

```
~/.mcpkit/
├── bundles/
│   ├── github.com/
│   │   └── org/
│   │       └── weather-tool/
│   │           └── v1.0.0/
│   │               ├── module.wasm      # Original WASM
│   │               ├── module.cwasm     # Precompiled cache
│   │               ├── config.yaml      # Configuration
│   │               └── metadata.json    # Pull metadata
│   └── registry.json                    # Local registry cache
└── cache/
    └── downloads/                       # Temporary download cache
```

## Implementation Details

### OCI Registry Integration

We use the `oci-distribution` crate to interact with OCI registries:

```rust
use oci_distribution::{Client, Reference};

pub struct BundleClient {
    oci_client: Client,
    cache_dir: PathBuf,
}

impl BundleClient {
    pub async fn push(&self, wasm: &[u8], config: &[u8], uri: &str) -> Result<String> {
        let reference = Reference::from_str(uri)?;

        // Create OCI manifest with two layers
        let manifest = Manifest {
            layers: vec![
                Layer::new(wasm, "application/wasm"),
                Layer::new(config, "application/vnd.mcpkit.config+yaml"),
            ],
            ..Default::default()
        };

        // Push to registry
        let digest = self.oci_client.push(&reference, &manifest).await?;
        Ok(digest)
    }

    pub async fn pull(&self, uri: &str) -> Result<Bundle> {
        let reference = Reference::from_str(uri)?;

        // Pull manifest
        let manifest = self.oci_client.pull_manifest(&reference).await?;

        // Pull layers
        let wasm = self.oci_client.pull_blob(&manifest.layers[0].digest).await?;
        let config = self.oci_client.pull_blob(&manifest.layers[1].digest).await?;

        // Verify digests
        verify_digest(&wasm, &manifest.layers[0].digest)?;
        verify_digest(&config, &manifest.layers[1].digest)?;

        // Cache locally
        self.cache_bundle(&reference, &wasm, &config)?;

        Ok(Bundle { wasm, config })
    }
}
```

### Configuration Structure

The existing config.yaml gets a new optional `distribution` section:

```yaml
# Standard mcpkit config sections
version: "1.0"
server:
  name: weather-tool
  version: 1.0.0

mcp:
  tools:
    - name: get_weather
      description: Get weather data
      input_schema:
        type: object
        properties:
          location:
            type: string

# NEW: Distribution section for publishing
distribution:
  # OCI registry URI for this bundle
  registry: "ghcr.io/myorg/weather-tool"

  # Version (defaults to server.version)
  version: "1.0.0"

  # Tags to apply
  tags: ["latest", "v1.0.0"]

  # Bundle metadata
  metadata:
    authors: ["Alice <alice@example.com>"]
    license: "MIT"
    repository: "https://github.com/myorg/weather-tool"
    keywords: ["weather", "api", "mcp"]

  # Files to include (defaults to module.wasm + config.yaml)
  include:
    - module.wasm
    - config.yaml
    - README.md  # Optional additional files

  # Registry authentication (can use env vars)
  auth:
    username: "${GITHUB_USER}"
    password: "${GITHUB_TOKEN}"
```

### Publishing Flow

```bash
# 1. Build WASM module
cargo build --target wasm32-wasi --release

# 2. Publish to registry using config.yaml
mcpkit bundle push --config config.yaml

# Or specify explicitly
mcpkit bundle push module.wasm config.yaml oci://ghcr.io/org/tool:latest

# 3. List published bundles
mcpkit bundle list --registry ghcr.io/org
```

### Consumption Flow

```bash
# 1. Pull bundle from registry
mcpkit bundle pull oci://ghcr.io/org/weather-tool:latest

# 2. Run directly from bundle
mcpkit server --from-bundle weather-tool

# 3. Or use in config
mcpkit server --config ~/.mcpkit/bundles/github.com/org/weather-tool/v1.0.0/config.yaml
```

## Comparison with Wassette

### Similarities

1. **OCI Registry Usage** - Both use OCI registries for distribution
2. **Multi-Layer Support** - Both use OCI layers for components
3. **GitHub Integration** - Both leverage GitHub Container Registry
4. **Digest Verification** - Both verify SHA256 digests
5. **Local Caching** - Both cache modules locally

### Differences

| Aspect | Wassette | mcpkit-rs |
|--------|----------|-----------|
| **Bundle Complexity** | WASM + policy + multiple metadata layers | WASM + config.yaml only |
| **Configuration** | Separate manifest files | Single config.yaml with distribution section |
| **Registry** | Curated component-registry.json | No central registry, direct URIs |
| **Tools** | Uses wkg CLI from Bytecode Alliance | Native Rust implementation |
| **Signature** | Cosign signatures required | Optional (future) |
| **Language** | JavaScript/TypeScript focused | Rust/WASM native |
| **Policy** | Separate policy layer | Embedded in config.yaml |
| **Compilation** | Runtime compilation | Precompiled caching |

### Trade-offs

**mcpkit-rs Advantages:**
- Simpler bundle format (2 files vs many)
- No manifest duplication
- Native Rust performance
- Faster startup with precompiled cache
- Single configuration source

**Wassette Advantages:**
- More mature ecosystem (wkg tools)
- Signature verification built-in
- Centralized registry for discovery
- Richer metadata format
- JavaScript ecosystem integration

## Security Considerations

### Digest Verification

All pulled artifacts are verified against their OCI manifest digests:

```rust
fn verify_digest(content: &[u8], expected: &str) -> Result<()> {
    use sha2::{Sha256, Digest};

    let mut hasher = Sha256::new();
    hasher.update(content);
    let computed = format!("sha256:{}", hex::encode(hasher.finalize()));

    if computed != expected {
        return Err(Error::DigestMismatch { expected, computed });
    }
    Ok(())
}
```

### Registry Authentication

Support for multiple authentication methods:

1. **Environment Variables**
   ```bash
   export GITHUB_TOKEN=ghp_xxxx
   mcpkit bundle push --registry ghcr.io/org/tool
   ```

2. **Config File**
   ```yaml
   distribution:
     auth:
       username: "${GITHUB_USER}"
       password: "${GITHUB_TOKEN}"
   ```

3. **Docker Config**
   ```bash
   docker login ghcr.io
   mcpkit bundle push  # Uses ~/.docker/config.json
   ```

### Trust Model

1. **Registry Trust** - Trust the OCI registry (GitHub, Docker Hub)
2. **Transport Security** - HTTPS only for registry communication
3. **Content Verification** - SHA256 digest verification
4. **No Code Execution** - WASM modules are sandboxed

Future: Add optional Cosign signature verification for higher security needs.

## Implementation Plan

### Phase 1: Core Infrastructure

**Week 1:**
- Add distribution section to Config struct in mcpkit-rs-config
- Create bundle module in mcpkit-rs for distribution logic
- Define Bundle, BundleClient, and error types

**Deliverables:**
- `crates/mcpkit-rs-config/src/distribution.rs`
- `crates/mcpkit-rs/src/bundle/mod.rs`

### Phase 2: OCI Integration

**Week 2:**
- Integrate oci-distribution crate
- Implement push/pull operations
- Add digest verification
- Implement local caching

**Deliverables:**
- `crates/mcpkit-rs/src/bundle/oci.rs`
- `crates/mcpkit-rs/src/bundle/cache.rs`

### Phase 3: CLI Commands

**Week 3:**
- Add bundle subcommands to CLI
- Implement push, pull, list commands
- Add --from-bundle flag to server command
- Authentication handling

**Deliverables:**
- `examples/cli/src/commands/bundle.rs`
- Updated CLI with bundle support

### Phase 4: Testing

**Week 4:**
- Unit tests for all components
- Integration tests with test registry
- End-to-end workflow tests
- Documentation and examples

**Deliverables:**
- `crates/mcpkit-rs/tests/bundle_tests.rs`
- `examples/bundle-example/`

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_oci_uri() {
        let uri = "oci://ghcr.io/org/tool:v1.0.0";
        let reference = Reference::from_str(uri).unwrap();
        assert_eq!(reference.registry(), "ghcr.io");
        assert_eq!(reference.repository(), "org/tool");
        assert_eq!(reference.tag(), Some("v1.0.0"));
    }

    #[test]
    fn test_digest_verification() {
        let content = b"test content";
        let digest = compute_digest(content);
        assert!(verify_digest(content, &digest).is_ok());
        assert!(verify_digest(b"wrong", &digest).is_err());
    }

    #[test]
    fn test_bundle_cache_path() {
        let cache = BundleCache::new("~/.mcpkit/bundles");
        let path = cache.bundle_path("ghcr.io/org/tool:v1.0.0");
        assert_eq!(path, PathBuf::from("~/.mcpkit/bundles/ghcr.io/org/tool/v1.0.0"));
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_push_pull_cycle() {
    // Start local OCI registry for testing
    let registry = test_registry::start().await;

    // Create test bundle
    let wasm = include_bytes!("../fixtures/test.wasm");
    let config = include_bytes!("../fixtures/config.yaml");

    // Push bundle
    let client = BundleClient::new();
    let uri = format!("oci://localhost:{}/test:latest", registry.port());
    let digest = client.push(wasm, config, &uri).await.unwrap();

    // Pull bundle
    let bundle = client.pull(&uri).await.unwrap();
    assert_eq!(bundle.wasm, wasm);
    assert_eq!(bundle.config, config);

    // Verify cached
    let cache_path = client.cache_dir.join("localhost").join("test/latest");
    assert!(cache_path.exists());
}

#[tokio::test]
async fn test_github_registry() {
    // Requires GITHUB_TOKEN env var
    if env::var("GITHUB_TOKEN").is_err() {
        return; // Skip in CI without token
    }

    let client = BundleClient::new();

    // Test pulling a known public bundle
    let bundle = client.pull("oci://ghcr.io/mcpkit/example:latest").await.unwrap();
    assert!(!bundle.wasm.is_empty());

    // Verify config parses
    let config: Config = serde_yaml::from_slice(&bundle.config).unwrap();
    assert_eq!(config.version, "1.0");
}
```

### End-to-End Tests

```bash
#!/bin/bash
# test/e2e/distribution.sh

set -e

# Build test WASM module
cd test/fixtures/weather-tool
cargo build --target wasm32-wasi --release

# Create config with distribution section
cat > config.yaml <<EOF
version: "1.0"
server:
  name: weather-tool
  version: 1.0.0

mcp:
  tools:
    - name: get_weather
      description: Get weather
      input_schema:
        type: object

distribution:
  registry: "localhost:5000/test/weather"
  tags: ["latest", "test"]
EOF

# Start local registry
docker run -d -p 5000:5000 --name test-registry registry:2

# Push bundle
mcpkit bundle push --config config.yaml

# Pull bundle
mcpkit bundle pull oci://localhost:5000/test/weather:latest

# Run from bundle
mcpkit server --from-bundle weather-tool &
SERVER_PID=$!

# Test MCP functionality
npm test test/mcp-client.js

# Cleanup
kill $SERVER_PID
docker stop test-registry
docker rm test-registry
```

### Performance Tests

```rust
#[bench]
fn bench_bundle_cache_lookup(b: &mut Bencher) {
    let cache = BundleCache::new_with_size(1000);

    // Populate cache
    for i in 0..1000 {
        cache.insert(&format!("bundle-{}", i), Bundle::dummy());
    }

    b.iter(|| {
        cache.get("bundle-500")
    });
}

#[bench]
fn bench_digest_verification(b: &mut Bencher) {
    let content = vec![0u8; 1024 * 1024]; // 1MB
    let digest = compute_digest(&content);

    b.iter(|| {
        verify_digest(&content, &digest).unwrap()
    });
}
```

## Summary

The mcpkit-rs distribution system provides a minimal, efficient way to share WASM MCP modules using existing OCI infrastructure. By embedding distribution metadata in the standard config.yaml and keeping bundles simple (WASM + config), we avoid complexity while enabling easy sharing through standard container registries.

Key advantages:
- No new infrastructure needed
- Single source of configuration
- Native Rust performance
- Simple, two-file bundles
- Registry agnostic

This approach balances simplicity with functionality, providing the essential features for WASM module distribution without the overhead of a full package management system.