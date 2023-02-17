function info() {
	echo "$@"
}

BUNDLE_DIR="$(readlink -f "$PWD/../src-tauri/target/release/bundle")"
MACOS_DMG="$(find "$BUNDLE_DIR/dmg" -depth 1 -type f -name "*.dmg")"
MACOS_UPDATER="$(find "$BUNDLE_DIR/macos" -depth 1 -type f -name "*.tar.gz")"
MACOS_UPDATER_SIG="$(find "$BUNDLE_DIR/macos" -depth 1 -type f -name "*.tar.gz.sig")"

info "$BUNDLE_DIR"
info
info "release built:"
info "  - $MACOS_DMG"
info "  - $MACOS_UPDATER"
info "  - $MACOS_UPDATER_SIG"
info

RELEASE_DIR="$PWD/../release"
mkdir -p "$RELEASE_DIR"

