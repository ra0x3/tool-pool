# WASM Configuration Guide for mcpkit-rs

## Table of Contents

- [Overview](#overview)
- [Runtime Configuration](#runtime-configuration)
- [Metering Configuration](#metering-configuration)
- [Policy Configuration](#policy-configuration)
- [Examples](#examples)

## Overview

The mcpkit-rs WASM runtime supports comprehensive configuration through YAML files, providing control over execution limits, resource metering, and security policies.

## Runtime Configuration

### Basic Runtime Settings

```yaml
runtime:
  type: wasmtime  # or wasmedge
  wasm:
    module_path: ./path/to/module.wasm
    memory_pages: 16  # 16 pages = 1MB (64KB per page)
    cache: true
    cache_dir: ./.wasm-cache
```

### Resource Limits

```yaml
runtime:
  limits:
    cpu: "100m"          # 100 millicores (0.1 CPU)
    memory: "512Mi"      # 512 MiB
    execution_time: "30s"  # 30 seconds timeout
    max_requests_per_minute: 1000
```

## Metering Configuration

The metering system provides real-time tracking and reporting of WASM compute usage with support for both Wasmtime (fuel) and WasmEdge (gas) runtimes.

### Basic Metering

```yaml
runtime:
  wasm:
    # Metering configuration
    metering:
      enabled: true                    # Enable compute metering
      max_compute_units: 10_000_000   # Maximum compute units allowed
      display_format: minimal         # minimal, detailed, json, progress
```

### Advanced Metering Options

```yaml
runtime:
  wasm:
    metering:
      enabled: true
      max_compute_units: 100_000_000  # 100M compute units

      # Memory limits (defense-in-depth with fuel)
      memory_limits:
        max_memory: "512Mi"           # Human-readable format
        soft_limit: "400Mi"           # Warning threshold
        max_tables: 10                # WASM table elements
        max_instances: 100            # Module instances

      # Real-time monitoring
      enable_monitoring: true          # Enable live updates

      # Sampling configuration for monitoring
      sampling:
        unit_threshold: 100_000       # Sample every 100K units
        time_threshold_ms: 100        # Sample every 100ms
        adaptive: true                # Adjust based on execution speed

      # Display format for CLI output
      display_format: minimal         # Options: minimal, detailed, json, progress

      # Enforcement mode
      enforcement: strict             # Options: tracking, warning, strict

      # Soft and hard limits
      limits:
        soft_limit: 80_000_000        # Warn at 80M units
        hard_limit: 100_000_000       # Stop at 100M units

      # Usage quotas
      quotas:
        per_request: 10_000_000       # 10M per request
        per_minute: 100_000_000       # 100M per minute
        per_hour: 5_000_000_000       # 5B per hour
        per_day: 100_000_000_000      # 100B per day
```

### Metering Display Formats

| Format | Description | Example Output |
|--------|-------------|----------------|
| `minimal` | Single-line abbreviated | `⚡ 1.2M CU` |
| `detailed` | Multi-line report | Full usage report with time, rate |
| `json` | JSON for programmatic use | `{"compute_units": 1234567, ...}` |
| `progress` | Progress bar visualization | `[████░░░░] 80% 8M CU` |

### Enforcement Modes

```yaml
# Tracking only - no limits enforced
enforcement: tracking

# Warning mode - warn at threshold
enforcement:
  warning:
    threshold: 0.8  # Warn at 80% of limit

# Strict mode - hard stop at limit
enforcement: strict
```

### Human-Readable Memory Formats

The system supports human-readable memory size formats:

- `Ki` - Kibibytes (1024 bytes)
- `Mi` - Mebibytes (1024 × 1024 bytes)
- `Gi` - Gibibytes (1024 × 1024 × 1024 bytes)

Examples:
- `512Ki` = 524,288 bytes
- `256Mi` = 268,435,456 bytes
- `2Gi` = 2,147,483,648 bytes

## Policy Configuration

### Security Policy Integration

```yaml
policy:
  version: "1.0"
  description: "Security policy with metering"

  core:
    resources:
      limits:
        cpu: "500m"
        memory: "512Mi"
        execution_time: "30s"

  # MCP-specific permissions
  mcp:
    tools:
      allow:
        - name: "calculator/*"
          max_calls_per_minute: 100
```

## Examples

### Development Configuration

Optimized for debugging with detailed metering output:

```yaml
runtime:
  type: wasmtime
  wasm:
    metering:
      enabled: true
      max_compute_units: 1_000_000_000  # 1B units (generous)
      display_format: detailed           # Show full metrics
      enforcement: tracking               # Don't enforce, just track
      enable_monitoring: true             # Live updates
      sampling:
        unit_threshold: 10_000           # Frequent sampling
        time_threshold_ms: 50             # 20 FPS updates
```

### Production Configuration

Optimized for performance with strict limits:

```yaml
runtime:
  type: wasmtime
  wasm:
    metering:
      enabled: true
      max_compute_units: 100_000_000    # 100M units
      display_format: minimal            # Minimal output
      enforcement: strict                # Hard limits
      enable_monitoring: false           # No overhead

      memory_limits:
        max_memory: "256Mi"
        max_tables: 5
        max_instances: 10

      limits:
        hard_limit: 100_000_000

      quotas:
        per_request: 10_000_000
        per_hour: 1_000_000_000
```

### Minimal Configuration

Basic metering with defaults:

```yaml
runtime:
  wasm:
    metering:
      enabled: true
      max_compute_units: 10_000_000
```

### High-Performance Configuration

For compute-intensive workloads:

```yaml
runtime:
  wasm:
    metering:
      enabled: true
      max_compute_units: 10_000_000_000  # 10B units

      # Adaptive sampling for high-speed execution
      sampling:
        adaptive: true
        unit_threshold: 1_000_000         # Less frequent sampling
        time_threshold_ms: 500             # 2 FPS updates

      # Large memory for data processing
      memory_limits:
        max_memory: "4Gi"
        soft_limit: "3Gi"
```

## CLI Integration

The metering configuration can be overridden via CLI flags:

```bash
# Enable metering with minimal display
mcpkit-rs --meter --meter-format minimal

# Enable metering with detailed output
mcpkit-rs --meter --meter-format detailed

# Set compute limit via CLI
mcpkit-rs --meter --max-compute-units 1000000

# Enable real-time monitoring
mcpkit-rs --meter --monitor
```

## Integration with Config Files

The metering configuration integrates with the existing ServerConfig:

```rust
use mcpkit_rs::config::ServerConfig;
use mcpkit_rs::wasm::MeteringConfig;

// Load config from YAML
let config = ServerConfig::from_file("config.yaml")?;

// Access metering config
if let Some(metering) = config.runtime.wasm.metering {
    println!("Max compute units: {:?}", metering.max_compute_units);
    println!("Display format: {:?}", metering.display_format);
}

// Create WasmContext with metering
let context = WasmContext::new()
    .with_metering(metering);
```

## Monitoring and Observability

### Real-time Monitoring

When `enable_monitoring` is true, the system provides live updates:

```
⚡ 1.2M CU @ 82.3M/s
⚡ 2.5M CU @ 95.1M/s
⚡ 3.8M CU @ 88.7M/s
```

### Final Metrics Report

After execution, detailed metrics are available:

```
Compute Usage Report
====================
Total:       3,845,291 CU
Abbreviated: 3.8M CU
Scientific:  3.845e6 CU
Time:        0.043s
Rate (avg):  89.4M/s
Rate (peak): 95.1M/s
```

## Best Practices

1. **Start with tracking mode** during development to understand actual usage
2. **Set soft limits** 20% below hard limits for early warning
3. **Use adaptive sampling** for variable workloads
4. **Disable monitoring in production** unless needed for debugging
5. **Use human-readable formats** for memory limits
6. **Configure per-request quotas** to prevent single-request DOS
7. **Log metrics** for capacity planning and optimization

## Troubleshooting

### Common Issues

1. **Fuel exhaustion errors**
   - Increase `max_compute_units`
   - Check for infinite loops in WASM code
   - Enable detailed metering to identify hotspots

2. **Memory limit exceeded**
   - Increase `memory_limits.max_memory`
   - Check for memory leaks
   - Monitor memory growth patterns

3. **Performance overhead**
   - Disable `enable_monitoring` in production
   - Increase sampling thresholds
   - Use `enforcement: tracking` instead of `strict`

4. **Quota violations**
   - Review quota settings
   - Implement request throttling
   - Consider per-user quotas

## Migration from Fuel-only Configuration

If you have existing configuration with just `fuel`:

```yaml
# Old configuration
runtime:
  wasm:
    fuel: 10000000

# New configuration
runtime:
  wasm:
    metering:
      enabled: true
      max_compute_units: 10000000
```

The system maintains backward compatibility - the old `fuel` field still works but the new `metering` section provides more control.