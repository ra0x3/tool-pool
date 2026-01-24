# Policy Benchmarks

## Running

```bash
# All benchmarks
cargo bench

# Specific benchmark
cargo bench --bench simple
cargo bench --bench practical
cargo bench --bench exhaustive
```

## Benchmarks

- **simple**: Basic operations (< 2ns per check)
- **practical**: Real-world scenarios (< 200ns per check)
- **exhaustive**: Stress tests (< 5μs worst case)

## Results

Tested on macOS Darwin 25.2.0

| Benchmark | Operation | Time |
|-----------|-----------|------|
| **Simple** | | |
| | Environment allowed | 1.8ns |
| | Environment denied | 1.0ns |
| | Policy compilation | 966ns |
| **Practical** | | |
| | Network checks | 1-30ns |
| | Storage allowed | 8.9µs |
| | Storage denied | 17ns |
| | Cached checks | 43-52ns |
| | Policy compilation | 47µs |
| **Exhaustive** | | |
| | Simple lookups | 4-38ns |
| | Complex patterns | 216µs-1.1ms |
| | Cache thrashing | 44ns |
| | Worst case | 90ns |
| | Compilation (500 rules) | 3.4ms |