# Setup Guide

This guide will help you set up the development environment for API Check.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Setup Methods](#setup-methods)
  - [Local Development](#local-development)
  - [Docker](#docker)
  - [Docker Compose](#docker-compose)
  - [Nix](#nix)
  - [GitHub Codespaces / Devcontainer](#github-codespaces--devcontainer)
- [Building the Project](#building-the-project)
- [Running Tests](#running-tests)
- [Common Issues](#common-issues)

## Prerequisites

Choose one of the following setup methods based on your preference:

- **Local Development**: Rust 1.70+
- **Docker**: Docker 20.10+ and Docker Compose (optional)
- **Nix**: Nix package manager with flakes enabled
- **Codespaces**: GitHub account (no local setup required)

## Quick Start

The fastest way to get started depends on your environment:

```bash
# Local development - start the server
cargo run

# Start with TUI dashboard
cargo run -- tui

# Run API tests
cargo run -- test -t http://example.com

# Docker
docker compose up

# Nix (with flakes)
nix develop
cargo run

# GitHub Codespaces
# Just open the repository in Codespaces - everything is preconfigured!
```

## Setup Methods

### Local Development

#### Installation

1. **Install Rust** (if not already installed):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env
   ```

2. **Clone and build**:
   ```bash
   git clone https://github.com/npv2k1/api-check.git
   cd api-check
   cargo build --release
   ```

3. **Run the application**:
   ```bash
   cargo run -- --help
   ```

#### Development Tools

Install additional development tools:

```bash
# Format checker
rustup component add rustfmt

# Linter
rustup component add clippy

# IDE support
rustup component add rust-analyzer
```

### Docker

#### Building and Running

1. **Build the Docker image**:
   ```bash
   docker build -t api-check:latest .
   ```

2. **Run the container**:
   ```bash
   # Run with help
   docker run --rm api-check:latest --help

   # Run server
   docker run --rm -p 3000:3000 api-check:latest server

   # Run with custom port
   docker run --rm -p 8080:8080 api-check:latest --port 8080 server
   ```

### Docker Compose

Docker Compose simplifies multi-service development and provides predefined configurations.

#### Running with Docker Compose

1. **Start the application**:
   ```bash
   docker compose up api-check
   ```

2. **Start in development mode**:
   ```bash
   docker compose up dev
   ```

3. **Build and run**:
   ```bash
   docker compose up --build
   ```

### Nix

Nix provides reproducible development environments across different systems.

#### Using Nix Flakes (Recommended)

1. **Enable flakes** (if not already enabled):
   ```bash
   mkdir -p ~/.config/nix
   echo "experimental-features = nix-command flakes" >> ~/.config/nix/nix.conf
   ```

2. **Enter development environment**:
   ```bash
   nix develop
   ```

3. **Build and run**:
   ```bash
   cargo build
   cargo run
   ```

### GitHub Codespaces / Devcontainer

The easiest way to get started with zero local setup.

#### Using GitHub Codespaces

1. Navigate to the repository on GitHub
2. Click "Code" → "Codespaces" → "Create codespace on main"
3. Wait for the environment to build
4. Start developing!

## Building the Project

### Development Build

```bash
cargo build
```

### Release Build (Optimized)

```bash
cargo build --release
```

The binary will be in `target/release/api-check`.

## Running Tests

### All Tests

```bash
cargo test
```

### With Output

```bash
cargo test -- --nocapture
```

## Code Quality

### Format Code

```bash
cargo fmt
```

### Run Linter

```bash
cargo clippy -- -D warnings
```

## Environment Variables

The application supports the following environment variables:

- `API_CHECK_SERVER_HOST`: Server host (default: `127.0.0.1`)
- `API_CHECK_SERVER_PORT`: Server port (default: `3000`)
- `API_CHECK_PROXY_ENABLED`: Enable proxy mode (default: `false`)
- `API_CHECK_PROXY_TARGET`: Proxy target URL
- `RUST_BACKTRACE`: Set to `1` for detailed error traces
- `RUST_LOG`: Set log level (e.g., `debug`, `info`, `warn`, `error`)

Example:

```bash
export API_CHECK_SERVER_PORT=8080
export RUST_LOG=debug
cargo run
```

## Common Issues

### Issue: OpenSSL not found

**Solution**: Install OpenSSL development libraries

```bash
# Ubuntu/Debian
sudo apt-get install libssl-dev pkg-config

# macOS
brew install openssl
export PKG_CONFIG_PATH="/usr/local/opt/openssl/lib/pkgconfig"

# Nix
nix develop  # Automatically provides OpenSSL
```

### Issue: Cargo is slow on first build

**Solution**: This is normal - Cargo downloads and compiles all dependencies on first build. Subsequent builds are much faster due to caching.

## Getting Help

If you encounter issues:

1. Check this guide's [Common Issues](#common-issues) section
2. Search existing [GitHub Issues](https://github.com/npv2k1/api-check/issues)
3. Open a new issue with:
   - Your setup method (Local/Docker/Nix/Codespaces)
   - Operating system and version
   - Rust version (`rustc --version`)
   - Complete error message
   - Steps to reproduce
