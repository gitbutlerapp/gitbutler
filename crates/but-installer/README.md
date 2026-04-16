# but-installer

A lightweight Rust installer for GitButler. Currently supports macOS only.

## How It Works

Installation is split into two parts:

1. **Bootstrap script** (`scripts/install.sh`) - Minimal shell script that:
   - Detects OS/architecture
   - Fetches installer metadata from `app.gitbutler.com`
   - Downloads the appropriate installer binary
   - Executes it

2. **Installer binary** (this crate) - Handles the actual installation:
   - Downloads and verifies the GitButler tarball
   - Extracts and installs the app bundle atomically
   - Sets up the `but` CLI symlink
   - Configures shell PATH and completions

## Usage

```bash
# Via bootstrap script (recommended)
curl -sSL https://gitbutler.com/install.sh | sh
curl -sSL https://gitbutler.com/install.sh | sh -s nightly
curl -sSL https://gitbutler.com/install.sh | sh -s 0.18.7

# Direct invocation
but-installer                  # Install latest stable
but-installer nightly          # Install nightly
but-installer 0.18.7           # Install specific version
```

## Building

```bash
cargo build --release -p but-installer
cargo test -p but-installer
```

## Flate2 benchmark baseline

The stored baseline combines a historical flate2 1.1.2 run (before
rust-lang/flate2-rs#502) and a current flate2 1.1.9 run for the same benchmark fixture.

Use the benchmark runner to compare the current backend against the stored baseline:

```bash
# Default rust_backend
cargo run --release -p but-installer --bin flate2-backend-benchmark -- \
  --suite current \
  --samples 5 \
  --baseline crates/but-installer/benchmarks/flate2-baseline.json

# Example for zlib-rs
cargo run --release -p but-installer --bin flate2-backend-benchmark \
  --no-default-features --features flate2-zlib-rs -- \
  --suite current \
  --samples 5 \
  --baseline crates/but-installer/benchmarks/flate2-baseline.json
```

The binary is optimized for size (~1.0MB) using system libcurl instead of bundling an HTTP client. Keeping this installer slim is a priority since it is downloaded before every installation.
