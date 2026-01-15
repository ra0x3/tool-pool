# Testing Guide for MCP Rust SDK

## Quick Start

### Running Tests Locally

To run tests locally, you need to have the required dependencies installed:

```bash
# Install test dependencies
make setup-env

# Run all tests
make test

# Run specific test suites
make test-python
make test-js
```

### Running Tests in Docker (Recommended)

If you don't want to install dependencies locally, you can run tests in a Docker container:

```bash
# Run all tests in Docker
make test-docker

# Run specific test suites in Docker
make test-python-docker
make test-js-docker
```

## Test Environment Requirements

### Required Dependencies

The full test suite requires the following dependencies:

- **Rust** (stable and nightly)
- **Node.js** (v20 or later) - for JavaScript integration tests
- **Python 3** - for Python integration tests
- **uv** - Python package manager (https://github.com/astral-sh/uv)

### Automatic Setup

Run the setup script to check and install dependencies:

```bash
./scripts/setup-test-env.sh
```

Or use Make:

```bash
make setup-env
```

## Test Categories

### Unit Tests

Basic Rust unit tests that don't require external dependencies:

```bash
cargo test --lib
```

### Integration Tests

Tests that require external services or language runtimes:

- **Python Tests** (`test_with_python`) - Requires Python 3 and uv
- **JavaScript Tests** (`test_with_js`) - Requires Node.js and npm

### Feature-Specific Tests

Tests can be run with different feature sets:

```bash
# Default features only
cargo test

# All features
cargo test --all-features

# Specific features
cargo test --features "client server"
```

## Docker Test Environment

The project includes a Docker setup for a fully reproducible test environment:

### Building the Test Image

```bash
docker-compose -f docker-compose.test.yml build
```

### Running Tests in Docker

```bash
# Run all tests
docker-compose -f docker-compose.test.yml run --rm test

# Run clippy
docker-compose -f docker-compose.test.yml run --rm clippy

# Check formatting
docker-compose -f docker-compose.test.yml run --rm fmt
```

### Docker Services Available

- `test` - Runs all tests with all features
- `test-python` - Runs Python integration tests
- `test-js` - Runs JavaScript integration tests
- `clippy` - Runs clippy linter
- `fmt` - Checks code formatting

## CI/CD Testing

The CI pipeline runs tests in a matrix configuration:

- Tests with default features
- Tests with all features
- Code formatting check (cargo +nightly fmt)
- Clippy linting (cargo clippy --locked --all-targets --all-features)
- Security audit
- Documentation generation

### Running CI Checks Locally

To run the same checks as CI:

```bash
# Run CI checks with local environment
make ci-local

# Run CI checks in Docker (exactly like CI)
make ci-docker
```

## Troubleshooting

### Python/JavaScript Tests Failing

If Python or JavaScript tests fail with "command not found" errors:

1. Check if dependencies are installed: `make setup-env`
2. Use Docker instead: `make test-docker`
3. Install manually:
   - Node.js: https://nodejs.org
   - Python: https://python.org
   - uv: `curl -LsSf https://astral.sh/uv/install.sh | sh`

### Tests Skipped vs Failed

- Tests will **fail** with a clear error message if dependencies are missing
- This is intentional to make it clear what needs to be installed
- In CI, all dependencies are pre-installed so tests should pass
- Locally, you can use Docker to avoid installing dependencies

### Docker Build Issues

If Docker builds are slow or failing:

1. Clean Docker volumes: `make clean`
2. Rebuild without cache: `docker-compose -f docker-compose.test.yml build --no-cache`
3. Check Docker disk space: `docker system df`

## Writing New Tests

### Adding Integration Tests

When adding tests that require external dependencies:

1. Add a check for the required command using `command_exists()`
2. Return a clear error message if the dependency is missing
3. Update this documentation with the new requirement
4. Ensure the dependency is installed in the Docker test image
5. Update CI configuration if needed

Example:

```rust
async fn command_exists(cmd: &str) -> bool {
    tokio::process::Command::new(if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    })
    .arg(cmd)
    .output()
    .await
    .map(|output| output.status.success())
    .unwrap_or(false)
}

#[tokio::test]
async fn test_requiring_tool() -> anyhow::Result<()> {
    if !command_exists("required-tool").await {
        eprintln!("Warning: required-tool is not installed.");
        return Err(anyhow::anyhow!("required-tool not available"));
    }
    // ... rest of test
}
```