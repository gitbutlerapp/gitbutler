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

The binary is optimized for size (~1.0MB) using system libcurl instead of bundling an HTTP client. Keeping this installer slim is a priority since it is downloaded before every installation.
