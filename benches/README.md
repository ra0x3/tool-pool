# mcpkit-rs Benchmarks

This directory contains performance benchmarks for the mcpkit-rs project.

## Available Benchmarks

The actual benchmark implementations are located in the individual crate directories:

### Policy Engine Benchmarks
Located in: `crates/mcpkit-rs-policy/benches/`

Three levels of complexity testing policy decision speed:
1. **Simple** - Basic operations with minimal rules (< 2ns per check)
2. **Practical** - Real-world scenarios with moderate complexity (< 200ns per check)
3. **Exhaustive** - Stress tests with hundreds of rules (< 5μs worst case)

See [crates/mcpkit-rs-policy/benches/README.md](../crates/mcpkit-rs-policy/benches/README.md) for details.

## Running Benchmarks

### Reproduce Benchmark Data

Three commands to reproduce the benchmark results:

```bash
# 1. Simple policy benchmark - basic operations
$ cargo bench --bench simple_policy_bench

# 2. Practical policy benchmark - real-world scenarios
$ cargo bench --bench practical_policy_bench

# 3. Exhaustive policy benchmark - stress testing
$ cargo bench --bench exhaustive_policy_bench
```

### Additional Options

```bash
# Run all policy benchmarks
$ cd crates/mcpkit-rs-policy
$ cargo bench

# Quick run for development
$ cargo bench -- --warm-up-time 1 --measurement-time 2

# Generate detailed HTML reports
$ cargo bench -- --verbose
# Reports saved to target/criterion/
```

## Performance Goals

| Benchmark Type | Target Performance |
|---------------|-------------------|
| Simple checks | < 50ns |
| Cached checks | < 100ns |
| Practical checks | < 200ns |
| Glob patterns | < 500ns |
| Worst case | < 5μs |
| Compilation (500 rules) | < 100μs |

## Adding New Benchmarks

To add benchmarks for other crates:
1. Add `criterion` to the crate's `[dev-dependencies]`
2. Create `benches/` directory in the crate
3. Add benchmark files and configure in `Cargo.toml`:
```toml
[[bench]]
name = "my_benchmark"
harness = false
```

## CI Integration

Add to your GitHub Actions workflow:
```yaml
- name: Run benchmarks
  run: |
    cargo bench --workspace

- name: Check for regression
  run: |
    cargo bench -- --baseline main
```