#!/usr/bin/env bash
set -euo pipefail

echo "Setting up test environment for MCP Rust SDK..."
echo ""

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check for Node.js
if command_exists node; then
    echo "✓ Node.js is installed ($(node --version))"
else
    echo "✗ Node.js is not installed"
    echo "  Please install Node.js from https://nodejs.org or using your package manager"
    echo "  macOS: brew install node"
    echo "  Ubuntu/Debian: sudo apt install nodejs npm"
    exit 1
fi

# Check for npm
if command_exists npm; then
    echo "✓ npm is installed ($(npm --version))"
else
    echo "✗ npm is not installed"
    echo "  npm should come with Node.js installation"
    exit 1
fi

# Check for Python
if command_exists python3; then
    echo "✓ Python3 is installed ($(python3 --version))"
else
    echo "✗ Python3 is not installed"
    echo "  Please install Python3 from https://python.org or using your package manager"
    echo "  macOS: brew install python3"
    echo "  Ubuntu/Debian: sudo apt install python3"
    exit 1
fi

# Check for uv (Python package manager)
if command_exists uv; then
    echo "✓ uv is installed ($(uv --version))"
else
    echo "✗ uv is not installed"
    echo "  Installing uv..."

    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS
        if command_exists brew; then
            brew install uv
        else
            curl -LsSf https://astral.sh/uv/install.sh | sh
        fi
    elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
        # Linux
        curl -LsSf https://astral.sh/uv/install.sh | sh
    else
        echo "  Please install uv manually from https://github.com/astral-sh/uv"
        exit 1
    fi

    # Verify installation
    if command_exists uv; then
        echo "  ✓ uv installed successfully"
    else
        echo "  ✗ Failed to install uv"
        echo "  You may need to add it to your PATH or restart your terminal"
        exit 1
    fi
fi

echo ""
echo "All test dependencies are installed!"
echo "You can now run the full test suite with:"
echo "  cargo test --all-features"