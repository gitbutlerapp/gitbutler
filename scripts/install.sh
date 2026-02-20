#!/bin/sh
# GitButler installer bootstrap script
# This script downloads and runs the GitButler installer binary

set -e

# Check for required commands
for cmd in curl mktemp grep sed uname chmod tr rm head; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        echo "Error: Required command '$cmd' not found. Please install it and try again." >&2
        exit 1
    fi
done

# Detect OS and architecture
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

# Map OS to installer format
case "$OS" in
    "darwin") INSTALLER_OS="macos" ;;
    "linux") INSTALLER_OS="linux" ;;
    *)
        echo "Error: This installer currently only supports macOS and Linux. Your OS: $OS" >&2
        exit 1
        ;;
esac

# Map architecture to installer format
case "$ARCH" in
    "x86_64") INSTALLER_ARCH="x86_64" ;;
    "arm64"|"aarch64") INSTALLER_ARCH="aarch64" ;;
    *)
        echo "Error: Unsupported architecture: $ARCH" >&2
        exit 1
        ;;
esac

# Create temp files and setup cleanup
# Use explicit templates for portability across GNU/BSD mktemp variants
INSTALLER_JSON=$(mktemp "${TMPDIR:-/tmp}/gitbutler-installer-json.XXXXXX")
INSTALLER_BIN=$(mktemp "${TMPDIR:-/tmp}/gitbutler-installer-bin.XXXXXX")
trap 'rm -f "$INSTALLER_JSON" "$INSTALLER_BIN"' EXIT INT TERM

# Fetch installer metadata and validate effective URL
INSTALLER_API_URL="https://app.gitbutler.com/installers/info/$INSTALLER_OS/$INSTALLER_ARCH"
echo "Fetching installer information..."

EFFECTIVE_URL=$(curl --fail --silent --show-error --location --max-redirs 5 --max-time 300 \
    -o "$INSTALLER_JSON" -w '%{url_effective}' "$INSTALLER_API_URL") || {
    echo "Error: Failed to fetch installer information from $INSTALLER_API_URL" >&2
    exit 1
}

case "$EFFECTIVE_URL" in
    https://app.gitbutler.com/*)
        : # Valid - stayed on trusted domain
        ;;
    *)
        echo "Error: API was redirected to an untrusted URL: $EFFECTIVE_URL" >&2
        exit 1
        ;;
esac

# Parse installer URL from JSON
INSTALLER_URL=$(grep -o '"url":"[^"]*"' "$INSTALLER_JSON" | head -1 | sed 's/"url":"\(.*\)"/\1/')

if [ -z "$INSTALLER_URL" ]; then
    echo "Error: Failed to parse installer URL from API response" >&2
    exit 1
fi

# Validate URL from API is trusted
case "$INSTALLER_URL" in
    https://releases.gitbutler.com/*)
        : # Valid
        ;;
    *)
        echo "Error: Installer URL is not from a trusted GitButler domain: $INSTALLER_URL" >&2
        exit 1
        ;;
esac

# Download installer and validate effective URL
echo "Downloading installer..."

EFFECTIVE_DOWNLOAD_URL=$(curl --fail --silent --show-error --location --max-redirs 5 --max-time 120 \
    -o "$INSTALLER_BIN" -w '%{url_effective}' "$INSTALLER_URL") || {
    echo "Error: Failed to download installer from $INSTALLER_URL" >&2
    exit 1
}

case "$EFFECTIVE_DOWNLOAD_URL" in
    https://releases.gitbutler.com/*)
        : # Valid - stayed on trusted domain
        ;;
    *)
        echo "Error: Download was redirected to an untrusted URL: $EFFECTIVE_DOWNLOAD_URL" >&2
        exit 1
        ;;
esac

# Verify downloaded installer
[ -s "$INSTALLER_BIN" ] || {
    echo "Error: Downloaded installer is empty" >&2
    exit 1
}

chmod +x "$INSTALLER_BIN" || {
    echo "Error: Failed to make installer executable" >&2
    exit 1
}

# Run installer with forwarded arguments
exec "$INSTALLER_BIN" "$@"
