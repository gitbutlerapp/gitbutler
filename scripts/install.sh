#!/bin/bash

set -euo pipefail
IFS=$'\n\t'

# Colors for output - only use if terminal supports it and colors aren't disabled
if [[ -t 1 ]] && [[ "${NO_COLOR:-}" == "" ]] && [[ "${TERM:-}" != "dumb" ]]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    BLUE='\033[0;34m'
    NC='\033[0m' # No Color
else
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    NC=''
fi

# Function to print error messages
error() {
    echo -e "${RED}Error: $1${NC}" >&2
    exit 1
}

# Function to print warning messages
warn() {
    echo -e "${YELLOW}Warning: $1${NC}" >&2
}

# Function to print success messages
success() {
    echo -e "${GREEN}$1${NC}"
}

# Function to print info messages
info() {
    echo -e "${BLUE}$1${NC}"
}

# Function to cleanup temp directory on exit
cleanup() {
    if [ -n "${TEMP_DIR:-}" ] && [ -d "$TEMP_DIR" ]; then
        rm -rf "$TEMP_DIR"
    fi
}
trap cleanup EXIT ERR INT TERM

# Verify HOME is set
if [ -z "${HOME:-}" ]; then
    error "HOME environment variable is not set"
fi

# Check for required commands
REQUIRED_COMMANDS=("curl" "tar" "uname" "grep" "sed" "basename" "find" "readlink")
MISSING_COMMANDS=()

for cmd in "${REQUIRED_COMMANDS[@]}"; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        MISSING_COMMANDS+=("$cmd")
    fi
done

if [ ${#MISSING_COMMANDS[@]} -gt 0 ]; then
    error "Required commands not found: ${MISSING_COMMANDS[*]}"
fi

# Check for optional but recommended commands
if ! command -v file >/dev/null 2>&1; then
    warn "Optional command 'file' not found - will skip archive type verification"
fi

# Detect OS and architecture
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

# Validate macOS
if [ "$OS" != "darwin" ]; then
    error "This installer currently only supports macOS. Your OS: $OS"
fi

# Map architecture to platform identifier
case $ARCH in
    "x86_64")
        PLATFORM="darwin-x86_64"
        ;;
    "arm64"|"aarch64")
        PLATFORM="darwin-aarch64"
        ;;
    *)
        error "Unsupported architecture: $ARCH"
        ;;
esac

info "Detected platform: $PLATFORM"

# Validate argument count
if [ $# -gt 1 ]; then
    error "Too many arguments. Usage: $0 [version] or GITBUTLER_VERSION=<version> $0"
fi

# Create temp directory
TEMP_DIR=$(mktemp -d -t gitbutler-install.XXXXXX)

# Fetch release information
# Support installing specific version via parameter or GITBUTLER_VERSION environment variable
# Parameter takes precedence over environment variable
REQUESTED_VERSION="${1:-${GITBUTLER_VERSION:-}}"

# Validate version parameter if provided
if [ -n "$REQUESTED_VERSION" ]; then
    # Reject if it looks like a flag
    if [[ "$REQUESTED_VERSION" == -* ]]; then
        error "Invalid version: $REQUESTED_VERSION. Usage: $0 [version] or GITBUTLER_VERSION=<version> $0"
    fi
    # Validate version format: only allow semver-compatible characters (alphanumeric, dots, hyphens, plus)
    # This prevents path traversal, query parameters, and other URL manipulation
    if [[ ! "$REQUESTED_VERSION" =~ ^[0-9a-zA-Z.+-]+$ ]]; then
        error "Invalid version format: $REQUESTED_VERSION. Version must contain only alphanumeric characters, dots, hyphens, and plus signs. Usage: $0 [version] or GITBUTLER_VERSION=<version> $0"
    fi
    RELEASES_URL="https://app.gitbutler.com/releases/version/$REQUESTED_VERSION"
    info "Fetching release information for version $REQUESTED_VERSION..."
else
    RELEASES_URL="https://app.gitbutler.com/releases"
    info "Fetching latest release information..."
fi

RELEASES_JSON="$TEMP_DIR/releases.json"
if ! curl --fail --location --silent --show-error -o "$RELEASES_JSON" "$RELEASES_URL"; then
    if [ -n "$REQUESTED_VERSION" ]; then
        error "Failed to fetch release information for version $REQUESTED_VERSION. Version may not exist."
    else
        error "Failed to fetch release information from $RELEASES_URL"
    fi
fi

# Parse JSON without jq - extract version
VERSION=$(grep -o '"version":"[^"]*"' "$RELEASES_JSON" | head -1 | sed 's/"version":"\(.*\)"/\1/')
if [ -z "$VERSION" ]; then
    error "Failed to parse version from release information"
fi

# Verify we got the version we requested
if [ -n "$REQUESTED_VERSION" ]; then
    if [ "$VERSION" != "$REQUESTED_VERSION" ]; then
        error "API returned version $VERSION but requested version $REQUESTED_VERSION"
    fi
    info "Installing version: $VERSION"
else
    info "Latest version: $VERSION"
fi

# Parse JSON without jq - extract download URL for platform
# Extract the platforms section and find our platform's URL
DOWNLOAD_URL=$(grep -o "\"$PLATFORM\":{[^}]*}" "$RELEASES_JSON" | grep -o '"url":"[^"]*"' | sed 's/"url":"\(.*\)"/\1/')
if [ -z "$DOWNLOAD_URL" ]; then
    error "Failed to find download URL for platform $PLATFORM"
fi

# Validate download URL for security
# Only allow HTTPS URLs from trusted GitButler domains
# Pattern allows: gitbutler.com, subdomain.gitbutler.com, but NOT .gitbutler.com
if [[ ! "$DOWNLOAD_URL" =~ ^https://(([a-zA-Z0-9-]+\.)+)?gitbutler\.com/ ]] && \
   [[ ! "$DOWNLOAD_URL" =~ ^https://releases\.gitbutler\.com/ ]]; then
    error "Download URL is not from a trusted GitButler domain: $DOWNLOAD_URL"
fi

info "Download URL: $DOWNLOAD_URL"

# Parse JSON without jq - extract signature for verification
SIGNATURE=$(grep -o "\"$PLATFORM\":{[^}]*}" "$RELEASES_JSON" | grep -o '"signature":"[^"]*"' | sed 's/"signature":"\(.*\)"/\1/')

# Extract filename from download URL to avoid hardcoding
FILENAME=$(basename "$DOWNLOAD_URL")
if [ -z "$FILENAME" ]; then
    error "Failed to extract filename from download URL"
fi

# Download tarball
info "Downloading GitButler $VERSION..."
TARBALL="$TEMP_DIR/$FILENAME"
if ! curl --fail --location --progress-bar -o "$TARBALL" "$DOWNLOAD_URL"; then
    error "Failed to download GitButler from $DOWNLOAD_URL"
fi

# Validate download - ensure file is not empty
if [ ! -s "$TARBALL" ]; then
    error "Downloaded file is empty"
fi

# Check if it's a valid gzip file (if file command is available)
if command -v file >/dev/null 2>&1; then
    if ! file "$TARBALL" | grep -q "gzip compressed"; then
        error "Downloaded file is not a valid gzip archive"
    fi
else
    # Fallback: check for gzip magic bytes (1f 8b) if file command not available
    if command -v od >/dev/null 2>&1; then
        MAGIC_BYTES=$(od -An -tx1 -N2 "$TARBALL" | tr -d ' ')
        if [ "$MAGIC_BYTES" != "1f8b" ]; then
            error "Downloaded file does not appear to be a valid gzip archive"
        fi
    else
        warn "Cannot verify archive type (file and od commands not available)"
    fi
fi

success "Download completed successfully"

# Verify signature with minisign
# GitButler's minisign public key (embedded for verification, decoded from crates/gitbutler-tauri/tauri.conf.json)
MINISIGN_PUBKEY="untrusted comment: minisign public key: 60576D8A3E4238EB
RWTrOEI+im1XYA9RBwyxnzFN/evFzJhU1lbQ70LVayWH3WRo7xQnRLD2"

if [ -z "$SIGNATURE" ]; then
    error "No signature found in release metadata - aborting for security"
fi

# Check if minisign is available
if command -v minisign >/dev/null 2>&1; then
    info "Verifying download signature with minisign..."

    # Write signature to temp file (decode from base64)
    SIGNATURE_FILE="$TEMP_DIR/$FILENAME.minisig"
    echo "$SIGNATURE" | base64 -d > "$SIGNATURE_FILE"

    # Write public key to temp file
    PUBKEY_FILE="$TEMP_DIR/minisign.pub"
    echo "$MINISIGN_PUBKEY" > "$PUBKEY_FILE"

    # Verify signature
    if minisign -Vm "$TARBALL" -p "$PUBKEY_FILE" -x "$SIGNATURE_FILE"; then
        success "Signature verification passed"
    else
        error "Signature verification failed - the download may have been tampered with"
    fi
else
    warn "minisign not found - skipping signature verification"
    info "For better security, install minisign: brew install minisign"
    info "See: https://jedisct1.github.io/minisign/"
fi

# Extract tarball
info "Extracting archive..."
if ! tar -xzf "$TARBALL" -C "$TEMP_DIR"; then
    error "Failed to extract archive"
fi

# Find the extracted .app bundle (should be GitButler.app but verify dynamically)
APP_DIR=$(find "$TEMP_DIR" -maxdepth 1 -name "*.app" -type d | head -1)
if [ -z "$APP_DIR" ]; then
    error "No .app bundle found in extracted archive"
fi

BINARIES_DIR="$APP_DIR/Contents/MacOS"
if [ ! -d "$BINARIES_DIR" ]; then
    error "Extracted app bundle does not contain expected directory structure (Contents/MacOS)"
fi

# Verify all three binaries exist
REQUIRED_BINARIES=("gitbutler-git-askpass" "gitbutler-git-setsid" "gitbutler-tauri")
for binary in "${REQUIRED_BINARIES[@]}"; do
    if [ ! -f "$BINARIES_DIR/$binary" ]; then
        error "Missing required binary: $binary"
    fi
done

success "Archive extracted successfully"

# Install app bundle
INSTALL_APP="$HOME/Applications/GitButler.app"
INSTALL_APP_BACKUP="$HOME/Applications/GitButler.app.backup"
INSTALL_APP_NEW="$HOME/Applications/GitButler.app.new"
BIN_DIR="$HOME/.local/bin"
info "Installing to $INSTALL_APP..."

# Atomic installation: install to temp location first, then swap
# This ensures we don't break an existing installation if something goes wrong

# Clean up any leftover temp files from previous failed installations
rm -rf "$INSTALL_APP_NEW" "$INSTALL_APP_BACKUP"

# Create Applications directory if it doesn't exist
mkdir -p "$HOME/Applications"

# Install to temporary location first
info "Installing to temporary location..."
cp -R "$APP_DIR" "$INSTALL_APP_NEW"

# Remove macOS quarantine attribute from the new app bundle
if ! xattr -dr com.apple.quarantine "$INSTALL_APP_NEW" 2>/dev/null; then
    warn "Could not remove quarantine attribute - macOS may show security warnings"
    info "If macOS blocks the app, go to System Settings > Privacy & Security and allow it"
fi

# Create bin directory for symlinks (outside the signed app bundle)
mkdir -p "$BIN_DIR"

# Check if 'but' already exists and warn if it's not our symlink
if [ -e "$BIN_DIR/but" ] && [ ! -L "$BIN_DIR/but" ]; then
    warn "A 'but' binary already exists at $BIN_DIR/but (not a symlink)"
    warn "This installation will replace it with a symlink to GitButler"
elif [ -L "$BIN_DIR/but" ]; then
    EXISTING_TARGET=$(readlink "$BIN_DIR/but")
    if [[ "$EXISTING_TARGET" != *"GitButler.app"* ]]; then
        warn "Found existing 'but' symlink pointing to: $EXISTING_TARGET"
        warn "This will be replaced with GitButler's 'but' command"
    fi
fi

# Create temporary symlink to test the new installation
NEW_APP_MACOS_DIR="$INSTALL_APP_NEW/Contents/MacOS"
ln -sf "$NEW_APP_MACOS_DIR/gitbutler-tauri" "$BIN_DIR/but.new"

# Verify the new installation works before replacing the old one
# First check if executable bit is set
if [ ! -x "$BIN_DIR/but.new" ]; then
    rm -rf "$INSTALL_APP_NEW" "$BIN_DIR/but.new"
    error "New installation verification failed - 'but' binary is not executable"
fi

# More robust check: actually run the binary to ensure it works
if ! "$BIN_DIR/but.new" --version >/dev/null 2>&1; then
    rm -rf "$INSTALL_APP_NEW" "$BIN_DIR/but.new"
    error "New installation verification failed - 'but' binary cannot run (may be corrupted or blocked by macOS)"
fi

# New installation is valid - now do the atomic swap
info "Swapping new installation into place..."

# Backup existing installation if it exists
if [ -d "$INSTALL_APP" ]; then
    mv "$INSTALL_APP" "$INSTALL_APP_BACKUP"
fi

# Move new installation into place
mv "$INSTALL_APP_NEW" "$INSTALL_APP"

# Update the symlink to point to the new installation
ln -sf "$INSTALL_APP/Contents/MacOS/gitbutler-tauri" "$BIN_DIR/but"
rm -f "$BIN_DIR/but.new"

# Verify final installation
# Check if executable and can actually run
if [ -x "$BIN_DIR/but" ] && "$BIN_DIR/but" --version >/dev/null 2>&1; then
    success "GitButler.app installed successfully"
    # Remove backup on success
    rm -rf "$INSTALL_APP_BACKUP"
else
    # Installation failed - try to restore backup
    warn "Final installation verification failed - attempting to restore backup"
    if [ -d "$INSTALL_APP_BACKUP" ]; then
        rm -rf "$INSTALL_APP"
        mv "$INSTALL_APP_BACKUP" "$INSTALL_APP"
        ln -sf "$INSTALL_APP/Contents/MacOS/gitbutler-tauri" "$BIN_DIR/but"

        # Verify the restored backup actually works
        if [ -x "$BIN_DIR/but" ] && "$BIN_DIR/but" --version >/dev/null 2>&1; then
            success "Backup was restored successfully"
            error "Installation failed but your previous installation was restored"
        else
            error "Installation failed and backup restoration also failed - 'but' command may not work"
        fi
    else
        error "Installation failed and no backup available to restore"
    fi
fi

success "GitButler CLI (but) installed successfully"

# Check if already in PATH
PATH_CMD="export PATH=\"$BIN_DIR:\$PATH\""

if [[ ":$PATH:" == *":$BIN_DIR:"* ]]; then
    success "$BIN_DIR is already in your PATH"
else
    # Detect shell config file based on which files exist
    # Note: We detect the user's preferred shell by checking for config files,
    # not by checking shell version variables (which reflect the shell running
    # this script, not the user's preferred shell)
    SHELL_CONFIG=""
    FISH_SHELL=false

    # Check for Fish shell first (requires different syntax)
    if [ -f "$HOME/.config/fish/config.fish" ]; then
        FISH_SHELL=true
    elif [ -f "$HOME/.zshrc" ]; then
        SHELL_CONFIG="$HOME/.zshrc"
    elif [ -f "$HOME/.bash_profile" ]; then
        SHELL_CONFIG="$HOME/.bash_profile"
    elif [ -f "$HOME/.bashrc" ]; then
        SHELL_CONFIG="$HOME/.bashrc"
    fi

    if [ "$FISH_SHELL" = true ]; then
        # Fish shell requires different syntax
        echo ""
        info "Fish shell detected. Please add the following to your ~/.config/fish/config.fish:"
        echo "  fish_add_path \$HOME/.local/bin"
        echo ""
        info "Or alternatively:"
        echo "  set -gx PATH \$HOME/.local/bin \$PATH"
    elif [ -n "$SHELL_CONFIG" ]; then
        # Check if PATH update already exists in config
        # Look for the exact PATH command we add, not just the directory name
        if grep -qsF "$PATH_CMD" "$SHELL_CONFIG"; then
            info "PATH configuration already exists in $SHELL_CONFIG"
        else
            # Check if we can write to the shell config file
            # If file exists, check if writable; if not exists, check if parent dir is writable
            CAN_WRITE=true
            if [ -f "$SHELL_CONFIG" ] && [ ! -w "$SHELL_CONFIG" ]; then
                CAN_WRITE=false
            elif [ ! -f "$SHELL_CONFIG" ] && [ ! -w "$(dirname "$SHELL_CONFIG")" ]; then
                CAN_WRITE=false
            fi

            if [ "$CAN_WRITE" = false ]; then
                warn "Cannot write to $SHELL_CONFIG (permission denied)"
                info "Please add the following line to your shell config file manually:"
                echo "  $PATH_CMD"
            else
                # Add to config file
                echo "" >> "$SHELL_CONFIG"
                echo "# Added by GitButler installer on $(date)" >> "$SHELL_CONFIG"
                echo "$PATH_CMD" >> "$SHELL_CONFIG"
                success "Updated $SHELL_CONFIG to include $BIN_DIR in PATH"
            fi
        fi
        echo ""
        info "To use 'but' in this terminal session, run:"
        echo "  source \"$SHELL_CONFIG\""
        echo ""
        info "Or close and reopen your terminal"
    else
        echo ""
        warn "Could not detect your shell configuration file"
        info "Please add the following line to your shell config file:"
        echo "  $PATH_CMD"
    fi
fi

echo ""
success "✓ GitButler CLI installation completed!"
echo ""
info "Usage:"
echo "  but --help           Show available commands"
echo "  but status           Show branch status"
echo "  but commit           Create a commit"
echo ""
info "For more information, visit: https://docs.gitbutler.com"
