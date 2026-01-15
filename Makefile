.PHONY: help test test-docker test-python test-js fmt clippy clean setup-env

# Default target
help:
	@echo "Available targets:"
	@echo "  setup-env    - Install test dependencies locally"
	@echo "  test         - Run all tests locally"
	@echo "  test-docker  - Run all tests in Docker container"
	@echo "  test-python  - Run Python integration tests"
	@echo "  test-js      - Run JavaScript integration tests"
	@echo "  fmt          - Check code formatting"
	@echo "  clippy       - Run clippy linter"
	@echo "  clean        - Clean build artifacts and caches"

# Setup local test environment
setup-env:
	@bash scripts/setup-test-env.sh

# Run all tests locally
test:
	cargo test --all-features

# Run tests in Docker container (with all dependencies)
test-docker:
	docker-compose -f docker-compose.test.yml run --rm test

# Run specific test suites in Docker
test-python-docker:
	docker-compose -f docker-compose.test.yml run --rm test-python

test-js-docker:
	docker-compose -f docker-compose.test.yml run --rm test-js

# Run specific test suites locally (will fail if deps not installed)
test-python:
	cargo test --all-features -p rmcp --test test_with_python

test-js:
	cargo test --all-features -p rmcp --test test_with_js

# Check formatting
fmt:
	cargo +nightly fmt --all -- --check

fmt-fix:
	cargo +nightly fmt --all

# Run clippy
clippy:
	cargo clippy --all-targets --all-features -- -D warnings

# Clean build artifacts
clean:
	cargo clean
	docker-compose -f docker-compose.test.yml down -v

# Build Docker test image
build-test-image:
	docker-compose -f docker-compose.test.yml build

# Run CI checks locally (mimics GitHub Actions)
ci-local: fmt clippy test
	@echo "✅ All CI checks passed!"

# Run CI checks in Docker (exactly like CI)
ci-docker:
	docker-compose -f docker-compose.test.yml run --rm fmt
	docker-compose -f docker-compose.test.yml run --rm clippy
	docker-compose -f docker-compose.test.yml run --rm test
	@echo "✅ All CI checks passed in Docker!"