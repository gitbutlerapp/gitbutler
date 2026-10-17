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

# From another script: skip the shell-config and agent-setup prompts even in a terminal
curl -sSL https://gitbutler.com/install.sh | GITBUTLER_NONINTERACTIVE=1 sh
```

## Building

```bash
cargo build --release -p but-installer
cargo test -p but-installer
```

The binary is about 5MB. It bundles `reqwest` with `rustls` instead of linking the system libcurl: `but update` uses this crate as a library, and libcurl would run its global initialisation at the start of every `but` process.
