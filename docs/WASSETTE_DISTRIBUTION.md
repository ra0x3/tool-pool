# Wassette Artifact Distribution System Documentation

## Overview

Wassette implements a sophisticated artifact distribution system for WebAssembly components based on the **OCI (Open Container Initiative)** specification. The system supports pulling components from OCI registries (primarily GitHub Container Registry), local files, and HTTP URLs, with built-in security verification, multi-layer support, and intelligent caching.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Distribution System                           │
├──────────────┬──────────────┬──────────────┬──────────────────┤
│   Registry   │  OCI Client  │   Storage    │    Manifest      │
│   (Catalog)  │ (Multi-layer)│   (Cache)    │  (Provisioning)  │
└──────────────┴──────────────┴──────────────┴──────────────────┘
```

## Core Components

### 1. Component Registry (`component-registry.json`)

A curated catalog of verified WebAssembly components:

```json
{
  "components": [
    {
      "name": "get-weather-js",
      "description": "Get the weather for a location",
      "uri": "oci://ghcr.io/microsoft/get-weather-js:latest"
    },
    {
      "name": "github-js",
      "description": "GitHub API integration via WebAssembly",
      "uri": "oci://ghcr.io/microsoft/github-js:latest"
    }
  ]
}
```

**Features:**
- 12 official Microsoft-published components
- Full-text search capability
- Case-insensitive name matching
- Direct URI references

### 2. OCI Multi-Layer Support

Wassette implements the **CNCF WebAssembly OCI Artifacts Specification**:

#### Media Types

```rust
// WebAssembly component layers
const WASM_LAYER_MEDIA_TYPES: [&str; 3] = [
    "application/wasm",
    "application/vnd.wasm.component.v1",
    "application/vnd.bytecodealliance.wasm.component.layer.v0+wasm",
];

// Policy layers (CNCF standard + legacy)
const POLICY_LAYER_MEDIA_TYPES: [&str; 4] = [
    "application/vnd.wasm.policy.v1+yaml",  // CNCF standard
    "application/vnd.wassette.policy+yaml",  // Legacy
    "application/x-yaml",
    "text/yaml",
];

// Configuration
const CONFIG_MEDIA_TYPES: [&str; 2] = [
    "application/vnd.wasm.config.v0+json",
    "application/vnd.oci.image.config.v1+json",
];
```

#### OCI Image Structure

```
OCI Artifact
├── Manifest (application/vnd.oci.image.manifest.v1+json)
│   ├── config (digest reference)
│   └── layers[]
│       ├── Layer 0: WASM component
│       └── Layer 1: Policy file (optional)
└── Config (application/vnd.wasm.config.v0+json)
    ├── architecture: "wasm"
    ├── os: "wasip1" or "wasip2"
    └── component metadata (exports, imports, target)
```

### 3. URI Schemes

Wassette supports three URI schemes for component loading:

#### File URI (`file://`)
```bash
# Local filesystem (absolute paths required)
wassette load --uri "file:///path/to/component.wasm"
```

#### OCI URI (`oci://`)
```bash
# GitHub Container Registry
wassette load --uri "oci://ghcr.io/microsoft/get-weather-js:latest"

# Docker Hub
wassette load --uri "oci://docker.io/myorg/component:v1.0.0"

# Azure Container Registry
wassette load --uri "oci://myregistry.azurecr.io/components/weather:latest"
```

#### HTTPS URI (`https://`)
```bash
# Direct download
wassette load --uri "https://example.com/components/my-component.wasm"
```

## Distribution Flow

### Publishing Components

#### 1. Build Phase
```bash
# Compile Rust component
cargo component build --release

# Or JavaScript component
npm run build
componentize-js --wit-path ./wit --world-name example build/index.js
```

#### 2. Package Phase
```bash
# Using wkg CLI (WebAssembly Package Manager)
wkg oci push \
  --manifest ./manifest.json \
  ./component.wasm \
  oci://ghcr.io/microsoft/my-component:latest
```

#### 3. Sign Phase (Keyless with Cosign)
```bash
# Sign the published artifact
cosign sign --yes ghcr.io/microsoft/my-component@sha256:abc123...
```

### Automated CI/CD Pipeline

```yaml
# .github/workflows/publish.yml
name: Publish Components
on:
  release:
    types: [published]

jobs:
  publish:
    strategy:
      matrix:
        component: [weather, github, memory]
    steps:
      - name: Build Component
        run: cargo component build --release

      - name: Publish to GHCR
        uses: bytecodealliance/wkg-github-action@v5
        with:
          file: target/wasm32-wasip1/release/${{ matrix.component }}.wasm
          oci-reference-without-tag: ghcr.io/${{ github.repository_owner }}/${{ matrix.component }}
          version: ${{ github.ref_name }}

      - name: Sign with Cosign
        run: |
          cosign sign --yes \
            ghcr.io/${{ github.repository_owner }}/${{ matrix.component }}@${{ steps.publish.outputs.digest }}
```

### Consuming Components

#### 1. Discovery
```bash
# Search registry
wassette registry search weather

# List all components
wassette registry list
```

#### 2. Load Process

```rust
// Internal flow
async fn load_component(uri: &str) -> Result<ComponentLoadOutcome> {
    // 1. Parse URI scheme
    let resource = match parse_uri_scheme(uri) {
        "file" => load_from_file(uri).await?,
        "oci" => load_from_oci(uri).await?,
        "https" => load_from_https(uri).await?,
    };

    // 2. Extract layers (WASM + optional policy)
    let (wasm_bytes, policy) = extract_layers(resource)?;

    // 3. Verify digests
    verify_sha256_digest(&wasm_bytes, expected_digest)?;

    // 4. Install to storage
    storage.install_component(component_id, wasm_bytes, policy)?;

    // 5. Compile or load from cache
    let compiled = compile_or_cache(component_id)?;

    // 6. Extract tool schemas
    let tools = extract_tools(&compiled)?;

    // 7. Register in tool map
    register_tools(component_id, tools)?;

    Ok(ComponentLoadOutcome {
        component_id,
        status: LoadResult::New,
        tool_names
    })
}
```

## Storage Architecture

### Directory Structure

```
wassette-root/
├── downloads/                      # Temporary download staging
│   └── temp-xxxx/
│       ├── component.wasm
│       └── component.policy.yaml
│
└── components/
    └── weather-service/
        ├── weather-service.wasm           # Component binary
        ├── weather-service.policy.yaml    # Security policy
        ├── weather-service.metadata.json  # Tool schemas & metadata
        ├── weather-service.cwasm          # Precompiled cache
        └── weather-service.policy.meta.json # Policy metadata
```

### Validation Stamps

Components are validated using stamps to detect changes:

```rust
pub struct ValidationStamp {
    pub file_size: u64,
    pub mtime: u64,                 // Modification time
    pub content_hash: Option<String>, // SHA-256 hash
}
```

### Component Metadata

Cached metadata for fast startup:

```rust
pub struct ComponentMetadata {
    pub tools: Vec<ToolSchema>,
    pub validation_stamp: ValidationStamp,
    pub created_at: SystemTime,
    pub component_info: Option<ComponentInfo>,
}

pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub normalized_name: String,  // Sanitized for MCP
}
```

## Multi-Layer OCI Implementation

### Layer Extraction

```rust
pub async fn pull_multi_layer_oci_artifact(
    reference: &str,
    oci_client: &oci_client::Client,
) -> Result<MultiLayerArtifact> {
    // 1. Pull manifest
    let manifest = pull_manifest(reference, oci_client).await?;

    // 2. Verify manifest digest
    verify_manifest_digest(&manifest, expected_digest)?;

    // 3. Pull config blob
    let config = pull_config_blob(&manifest.config, oci_client).await?;

    // 4. Download layers
    let mut wasm_layer = None;
    let mut policy_layer = None;

    for layer in &manifest.layers {
        match layer.media_type.as_str() {
            t if WASM_LAYER_MEDIA_TYPES.contains(&t) => {
                wasm_layer = Some(pull_blob(&layer.digest, oci_client).await?);
            }
            t if POLICY_LAYER_MEDIA_TYPES.contains(&t) => {
                policy_layer = Some(pull_blob(&layer.digest, oci_client).await?);
            }
            _ => {} // Ignore unknown layers
        }
    }

    // 5. Verify layer digests
    verify_layer_digest(&wasm_layer, &manifest.layers[0].digest)?;

    Ok(MultiLayerArtifact {
        wasm_component: wasm_layer.unwrap(),
        policy: policy_layer,
        config,
    })
}
```

### Security Verification

#### Digest Verification
```rust
fn verify_sha256_digest(content: &[u8], expected: &str) -> Result<()> {
    use sha2::{Sha256, Digest};

    let mut hasher = Sha256::new();
    hasher.update(content);
    let computed = format!("sha256:{}", hex::encode(hasher.finalize()));

    if computed != expected {
        bail!("Digest mismatch: expected {}, got {}", expected, computed);
    }
    Ok(())
}
```

#### Cosign Signature Verification
```bash
# Verify signature (automated in CI)
cosign verify \
  --certificate-identity-regexp "https://github.com/microsoft/wassette/*" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  ghcr.io/microsoft/get-weather-js:latest
```

## Caching Strategy

### Multi-Level Cache

1. **Registry Cache** (CDN/Registry level)
   - OCI registries provide built-in CDN caching
   - Pull-through cache proxies supported

2. **Download Cache** (Network level)
   ```rust
   // Concurrent download limiting
   const DEFAULT_DOWNLOAD_CONCURRENCY: usize = 8;

   let semaphore = Arc::new(Semaphore::new(max_concurrent_downloads));
   let permit = semaphore.acquire().await?;
   ```

3. **Component Cache** (Filesystem level)
   ```
   components/<id>/
   ├── component.wasm      # Original
   ├── component.cwasm     # Precompiled
   └── component.metadata  # Extracted info
   ```

4. **Compilation Cache** (Runtime level)
   ```rust
   // Serialize compiled module
   let compiled = engine.precompile_component(&wasm_bytes)?;
   fs::write(cache_path, compiled)?;

   // Deserialize on next load
   let compiled = unsafe {
       Component::deserialize(&engine, &cache_bytes)?
   };
   ```

5. **Metadata Cache** (Application level)
   - Tool schemas cached in JSON
   - Validation stamps for staleness detection
   - Fast startup without recompilation

### Cache Invalidation

```rust
fn is_cache_valid(source: &Path, cache: &Path) -> bool {
    // Check file existence
    if !cache.exists() { return false; }

    // Compare modification times
    let source_mtime = source.metadata()?.modified()?;
    let cache_mtime = cache.metadata()?.modified()?;

    // Check validation stamp
    let stamp = load_validation_stamp(cache)?;
    let current_size = source.metadata()?.len();
    let current_hash = compute_sha256(source)?;

    stamp.file_size == current_size &&
    stamp.content_hash == Some(current_hash) &&
    cache_mtime > source_mtime
}
```

## Manifest-Based Provisioning

### Declarative Component Management

```yaml
# manifest.yaml
version: 1
components:
  - uri: oci://ghcr.io/microsoft/get-weather-js:latest
    name: weather-service
    digest: sha256:abc123...  # Optional verification
    permissions:
      network:
        allow:
          - host: api.openweathermap.org
      environment:
        allow:
          - key: OPENWEATHER_API_KEY

  - uri: oci://ghcr.io/microsoft/github-js:latest
    name: github-api
    permissions:
      network:
        allow:
          - host: api.github.com
      environment:
        allow:
          - key: GITHUB_TOKEN
```

### Apply Manifest

```bash
# Load all components with permissions
wassette apply -f manifest.yaml

# With validation
wassette apply -f manifest.yaml --verify-digests
```

## Registry Operations

### Search Implementation

```rust
pub fn search_components(registry: &Registry, query: &str) -> Vec<Component> {
    let terms: Vec<&str> = query.split_whitespace().collect();

    registry.components
        .iter()
        .filter(|component| {
            terms.iter().all(|term| {
                let term_lower = term.to_lowercase();
                component.name.to_lowercase().contains(&term_lower) ||
                component.description.to_lowercase().contains(&term_lower) ||
                component.uri.to_lowercase().contains(&term_lower)
            })
        })
        .cloned()
        .collect()
}
```

### Registry Validation

Automated validation on PR:

```bash
#!/bin/bash
# validate-component-registry.sh

# Get changed URIs
NEW_URIS=$(git diff main...HEAD component-registry.json | \
           grep '^\+.*"uri"' | \
           sed 's/.*"uri": "\(.*\)".*/\1/')

# Test each component
for uri in $NEW_URIS; do
    echo "Testing $uri..."

    # Start Wassette server
    ./wassette serve --transport sse &
    SERVER_PID=$!

    # Test component load via MCP
    npx @modelcontextprotocol/inspector \
        sse://localhost:8089/sse \
        --test-component "$uri"

    # Check result
    if [ $? -eq 0 ]; then
        echo "✓ $uri loaded successfully"
    else
        echo "✗ $uri failed to load"
        exit 1
    fi

    kill $SERVER_PID
done
```

## Performance Optimizations

### 1. Parallel Downloads
```rust
// Download multiple components concurrently
let futures = components.iter().map(|uri| {
    async move {
        load_component(uri).await
    }
});

let results = futures::future::join_all(futures).await;
```

### 2. Streaming Decompression
```rust
// Stream and decompress layers
let reader = BufReader::new(response.bytes_stream());
let decoder = GzDecoder::new(reader);
let mut tar = Archive::new(decoder);
tar.unpack(target_dir)?;
```

### 3. Lazy Loading
```rust
// Only compile when first accessed
pub struct LazyComponent {
    source: PathBuf,
    compiled: OnceCell<Component>,
}

impl LazyComponent {
    pub fn get(&self) -> &Component {
        self.compiled.get_or_init(|| {
            compile_component(&self.source)
        })
    }
}
```

### 4. Incremental Updates
```rust
// Check if component needs update
if let Some(existing) = storage.get_component(id)? {
    if existing.digest == new_digest {
        return Ok(ComponentLoadOutcome {
            status: LoadResult::AlreadyLoaded,
            ..
        });
    }
}
```

## Error Handling

### Detailed Error Messages

```rust
pub enum DistributionError {
    OciPullFailed {
        reference: String,
        error: String
    },
    DigestMismatch {
        expected: String,
        computed: String
    },
    InvalidMediaType {
        found: String,
        expected: Vec<String>
    },
    NetworkTimeout {
        uri: String,
        timeout_secs: u64
    },
}

impl Display for DistributionError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            OciPullFailed { reference, error } => {
                write!(f, "Failed to pull OCI artifact '{}': {}\n\
                          Try: wassette login ghcr.io", reference, error)
            }
            DigestMismatch { expected, computed } => {
                write!(f, "Security validation failed!\n\
                          Expected: {}\n\
                          Computed: {}\n\
                          The component may have been tampered with.",
                          expected, computed)
            }
            // ... other cases
        }
    }
}
```

### Retry Logic

```rust
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY: Duration = Duration::from_secs(2);

async fn download_with_retry(uri: &str) -> Result<Vec<u8>> {
    let mut attempts = 0;
    loop {
        match download(uri).await {
            Ok(data) => return Ok(data),
            Err(e) if attempts < MAX_RETRIES => {
                attempts += 1;
                eprintln!("Download failed (attempt {}/{}): {}",
                         attempts, MAX_RETRIES, e);
                tokio::time::sleep(RETRY_DELAY).await;
            }
            Err(e) => return Err(e),
        }
    }
}
```

## Security Considerations

### 1. Supply Chain Security
- **Signed artifacts**: All official components are Cosign-signed
- **Digest verification**: SHA-256 verification at every layer
- **SBOM generation**: Software Bill of Materials for components

### 2. Registry Authentication
```bash
# Login to GitHub Container Registry
wassette login ghcr.io --username $GITHUB_USER --password $GITHUB_TOKEN

# Login to Docker Hub
wassette login docker.io --username $DOCKER_USER --password $DOCKER_TOKEN

# Use environment variables
export WASSETTE_REGISTRY_AUTH="ghcr.io:token:$GITHUB_TOKEN"
```

### 3. Network Security
- **TLS verification**: Required for HTTPS/OCI pulls
- **Proxy support**: HTTP_PROXY/HTTPS_PROXY environment variables
- **Private registries**: Support for self-hosted registries

### 4. Sandboxing
- Components run in WASI sandbox
- No direct filesystem/network access without policy
- Resource limits enforced via policies

## Best Practices

### 1. Version Pinning
```yaml
# Good: Specific version
uri: oci://ghcr.io/microsoft/weather:v1.2.3

# Good: Digest pinning (immutable)
uri: oci://ghcr.io/microsoft/weather@sha256:abc123...

# Avoid: Latest tag in production
uri: oci://ghcr.io/microsoft/weather:latest
```

### 2. Registry Mirroring
```bash
# Set up local mirror
docker run -d -p 5000:5000 \
  -e REGISTRY_PROXY_REMOTEURL=https://ghcr.io \
  registry:2

# Use mirror
wassette load --uri oci://localhost:5000/microsoft/weather:latest
```

### 3. Manifest Validation
```yaml
# Include digests for verification
components:
  - uri: oci://ghcr.io/microsoft/weather:v1.0.0
    digest: sha256:1234abcd...  # Always verify in production
```

### 4. Cache Management
```bash
# Clear compilation cache
rm -rf ~/.wassette/components/*.cwasm

# Clear entire cache
wassette cache clear

# Verify cache integrity
wassette cache verify
```

## Troubleshooting

### Common Issues

#### 1. OCI Pull Failures
```bash
# Check authentication
wassette login ghcr.io --debug

# Test with curl
curl -H "Authorization: Bearer $TOKEN" \
  https://ghcr.io/v2/microsoft/weather/manifests/latest

# Use verbose logging
RUST_LOG=debug wassette load --uri oci://...
```

#### 2. Digest Mismatches
```bash
# Manually verify digest
sha256sum component.wasm

# Pull specific digest
wassette load --uri oci://ghcr.io/microsoft/weather@sha256:...

# Skip verification (development only!)
wassette load --uri oci://... --skip-verification
```

#### 3. Cache Corruption
```bash
# Validate cache entries
wassette cache verify

# Force recompilation
wassette load --uri ... --force-compile

# Clear specific component
rm -rf ~/.wassette/components/weather-service/
```

## Future Enhancements

### Planned Features
- **P2P distribution**: BitTorrent-style component sharing
- **Delta updates**: Only download changed layers
- **Compression optimization**: Zstd compression for smaller artifacts
- **Offline mode**: Full offline operation with local registry
- **Component signing**: Developer key-based signatures

### Under Consideration
- IPFS integration for decentralized distribution
- Component dependency resolution
- Automatic security updates
- Registry federation
- Build reproducibility verification

## Summary

Wassette's artifact distribution system provides a robust, secure, and efficient mechanism for distributing WebAssembly components. By leveraging OCI standards, implementing multi-layer support, and providing comprehensive caching, it enables scalable deployment of WebAssembly workloads while maintaining security through digest verification and signature validation.

Key strengths:
- **Standards-based**: CNCF OCI WebAssembly specification
- **Secure**: Digest verification and Cosign signatures
- **Efficient**: Multi-level caching and parallel downloads
- **Flexible**: Multiple URI schemes and registry support
- **Developer-friendly**: Simple CLI and declarative manifests