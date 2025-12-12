#!/bin/bash

set -o errexit
set -o nounset
set -o pipefail

PWD="$(dirname "$(readlink -f -- "$0")")"

CHANNEL=""
DO_SIGN="false"
VERSION=""
TARGET="${CARGO_BUILD_TARGET:-}"

function help() {
	local to
	to="$1"

	echo "Usage: $0 <flags>" 1>&"$to"
	echo 1>&"$to"
	echo "flags:" 1>&"$to"
	echo "	--version											release version." 1>&"$to"
	echo "	--dist												path to store artifacts in." 1>&"$to"
	echo "	--sign												if set, will sign the app." 1>&"$to"
	echo "	--channel											the channel to use for the release (release | nightly)." 1>&"$to"
	echo "	--help												display this message." 1>&"$to"
}

function error() {
	echo "error: $*" 1>&2
	echo 1>&2
	help 2
	exit 1
}

function info() {
	echo "$@"
}

function os() {
	local os
	os="$(uname -s)"
	case "$os" in
	Darwin)
		echo "macos"
		;;
	Linux)
		echo "linux"
		;;
	Windows | MSYS* | MINGW*)
		echo "windows"
		;;
	*)
		error "$os: unsupported"
		;;
	esac
}

function arch() {
	local arch

	# If TARGET is specified, extract architecture from it
	if [ -n "${TARGET:-}" ]; then
		case "$TARGET" in
		*aarch64* | *arm64*)
			echo "aarch64"
			return
			;;
		*x86_64* | *amd64*)
			echo "x86_64"
			return
			;;
		esac
	fi

	# Otherwise, detect from system
	arch="$(uname -m)"
	case "$arch" in
	arm64 | aarch64)
		echo "aarch64"
		;;
	x86_64)
		echo "x86_64"
		;;
	*)
		error "$arch: unsupported architecture"
		;;
	esac
}

ARCH="$(arch)"
OS="$(os)"
DIST="release"

function tauri() {
	(cd "$PWD/.." && pnpm tauri-for-release "$@")
}

while [[ $# -gt 0 ]]; do
	case "$1" in
	--help)
		help 1
		exit 1
		;;
	--version)
		VERSION="$2"
		shift
		shift
		;;
	--dist)
		DIST="$2"
		shift
		shift
		;;
	--sign)
		DO_SIGN="true"
		shift
		;;
	--channel)
		CHANNEL="$2"
		shift
		shift
		;;
	*)
		error "unknown flag $1"
		;;
	esac
done

# Recalculate ARCH after TARGET is set
ARCH="$(arch)"

[ -z "${VERSION-}" ] && error "--version is not set"

[ -z "${TAURI_SIGNING_PRIVATE_KEY-}" ] && error "$TAURI_SIGNING_PRIVATE_KEY is not set"
[ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD-}" ] && error "$TAURI_SIGNING_PRIVATE_KEY_PASSWORD is not set"

if [ "$CHANNEL" != "release" ] && [ "$CHANNEL" != "nightly" ]; then
	error "--channel must be either 'release' or 'nightly'"
fi

if [ "$DO_SIGN" = "true" ]; then
	if [ "$OS" = "macos" ]; then
		[ -z "${APPLE_CERTIFICATE-}" ] && error "$APPLE_CERTIFICATE is not set"
		[ -z "${APPLE_CERTIFICATE_PASSWORD-}" ] && error "$APPLE_CERTIFICATE_PASSWORD is not set"
		[ -z "${APPLE_ID-}" ] && error "$APPLE_ID is not set"
		[ -z "${APPLE_TEAM_ID-}" ] && error "$APPLE_TEAM_ID is not set"
		[ -z "${APPLE_PASSWORD-}" ] && error "$APPLE_PASSWORD is not set"
		export APPLE_CERTIFICATE="$APPLE_CERTIFICATE"
		export APPLE_CERTIFICATE_PASSWORD="$APPLE_CERTIFICATE_PASSWORD"
		export APPLE_ID="$APPLE_ID"
		export APPLE_TEAM_ID="$APPLE_TEAM_ID"
		export APPLE_PASSWORD="$APPLE_PASSWORD"
	elif [ "$OS" == "linux" ]; then
		[ -z "${APPIMAGE_KEY_ID-}" ] && error "$APPIMAGE_KEY_ID is not set"
		[ -z "${APPIMAGE_KEY_PASSPHRASE-}" ] && error "$APPIMAGE_KEY_PASSPHRASE is not set"
		export SIGN=1
		export SIGN_KEY="$APPIMAGE_KEY_ID"
		export APPIMAGETOOL_SIGN_PASSPHRASE="$APPIMAGE_KEY_PASSPHRASE"
	elif [ "$OS" == "windows" ]; then
		# Nothing to do on windows
		export OS
	else
		error "signing is not supported on $(uname -s)"
	fi
fi

info "building:"
info "	channel: $CHANNEL"
info "	version: $VERSION"
info "	os: $OS"
info "	arch: $ARCH"
info "	dist: $DIST"
info "	sign: $DO_SIGN"
info "	target: ${TARGET:-default}"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' exit

CONFIG_PATH=$(readlink -f "$PWD/../crates/gitbutler-tauri/tauri.conf.$CHANNEL.json")

if [ "$OS" = "windows" ]; then
  # WARNING: `builtin-but` doesn't work on Windows, see https://github.com/gitbutlerapp/gitbutler/issues/11461.
  #          Should it be re-added, please ensure that `but` is built
  #          as part of the 'beforeBuildCommand' in tauri.conf AND it must be injected
  #          via 'inject-git-binaries.sh'.
  EXTERNAL_BIN='["gitbutler-git-setsid", "gitbutler-git-askpass", "but"]'
	FEATURES="windows"
else
  EXTERNAL_BIN='["gitbutler-git-setsid", "gitbutler-git-askpass"]'
	FEATURES="builtin-but"
fi

# update the version in the tauri release config
jq  --arg version "$VERSION"\
    --argjson externalBin "$EXTERNAL_BIN"\
  '.version = $version | .bundle.externalBin = $externalBin' "$CONFIG_PATH" >"$TMP_DIR/tauri.conf.json"

# Useful for understanding exactly what goes into the tauri build/bundle.
cat "$TMP_DIR/tauri.conf.json"

# set the VERSION and CHANNEL as an environment variables so that they available in the but CLI
export VERSION
export CHANNEL

# Build the app with release config
if [ -n "$TARGET" ]; then
	# Export TARGET for cargo to use
	export CARGO_BUILD_TARGET="$TARGET"

	# Build with specified target
	# Note: passing --target is necessary to let tauri find the binaries,
	# it ignores CARGO_BUILD_TARGET and is more of a hack.
	tauri build \
		--verbose \
		--features "$FEATURES" \
		--config "$TMP_DIR/tauri.conf.json" \
		--target "$TARGET"

  BUNDLE_DIR=$(readlink -f "$PWD/../target/$TARGET/release/bundle")
else
	# Build with default target
	tauri build \
		--verbose \
		--features "$FEATURES" \
		--config "$TMP_DIR/tauri.conf.json"

	BUNDLE_DIR=$(readlink -f "$PWD/../target/release/bundle")
fi

# The release dir determines a (significant portion of) the S3 object keys for
# the artifacts.
RELEASE_DIR="$DIST/$OS/$ARCH"
if [ "$OS" = "linux" ] && [ $(lsb_release -cs) = "noble" ]; then
  # Our default Linux build is Ubuntu jammy (22.04). We still build for noble
  # (24.04) but put the builds in a hidden-away part of the S3 bucket. The
  # primary reason for this build existing is that the noble-built AppImage
  # tends to work better with newer distros than the jammy-built AppImage.
  RELEASE_DIR="$DIST/$OS-$(lsb_release -cs)/$ARCH"
fi

echo "Resolved RELEASE_DIR=$RELEASE_DIR"

mkdir -p "$RELEASE_DIR"

if [ "$OS" = "macos" ]; then
	MACOS_DMG="$(find "$BUNDLE_DIR/dmg" -depth 1 -type f -name "*.dmg")"
	MACOS_UPDATER="$(find "$BUNDLE_DIR/macos" -depth 1 -type f -name "*.tar.gz")"
	MACOS_UPDATER_SIG="$(find "$BUNDLE_DIR/macos" -depth 1 -type f -name "*.tar.gz.sig")"

	cp "$MACOS_DMG" "$RELEASE_DIR"
	cp "$MACOS_UPDATER" "$RELEASE_DIR"
	cp "$MACOS_UPDATER_SIG" "$RELEASE_DIR"

	info "built:"
	info "	- $RELEASE_DIR/$(basename "$MACOS_DMG")"
	info "	- $RELEASE_DIR/$(basename "$MACOS_UPDATER")"
	info "	- $RELEASE_DIR/$(basename "$MACOS_UPDATER_SIG")"
elif [ "$OS" = "linux" ]; then
	APPIMAGE="$(find "$BUNDLE_DIR/appimage" -name \*.AppImage)"
	APPIMAGE_UPDATER="$(find "$BUNDLE_DIR/appimage" -name \*.AppImage.tar.gz)"
	APPIMAGE_UPDATER_SIG="$(find "$BUNDLE_DIR/appimage" -name \*.AppImage.tar.gz.sig)"
	DEB="$(find "$BUNDLE_DIR/deb" -name \*.deb)"
	RPM="$(find "$BUNDLE_DIR/rpm" -name \*.rpm)"

	cp "$APPIMAGE" "$RELEASE_DIR"
	cp "$APPIMAGE_UPDATER" "$RELEASE_DIR"
	cp "$APPIMAGE_UPDATER_SIG" "$RELEASE_DIR"
	cp "$DEB" "$RELEASE_DIR"
	cp "$RPM" "$RELEASE_DIR"

	info "built:"
	info "	- $RELEASE_DIR/$(basename "$APPIMAGE")"
	info "	- $RELEASE_DIR/$(basename "$APPIMAGE_UPDATER")"
	info "	- $RELEASE_DIR/$(basename "$APPIMAGE_UPDATER_SIG")"
	info "	- $RELEASE_DIR/$(basename "$DEB")"
	info "	- $RELEASE_DIR/$(basename "$RPM")"
elif [ "$OS" = "windows" ]; then
	WINDOWS_INSTALLER="$(find "$BUNDLE_DIR/msi" -name \*.msi)"
	WINDOWS_UPDATER="$(find "$BUNDLE_DIR/msi" -name \*.msi.zip)"
	WINDOWS_UPDATER_SIG="$(find "$BUNDLE_DIR/msi" -name \*.msi.zip.sig)"

	cp "$WINDOWS_INSTALLER" "$RELEASE_DIR"
	cp "$WINDOWS_UPDATER" "$RELEASE_DIR"
	cp "$WINDOWS_UPDATER_SIG" "$RELEASE_DIR"

	info "built:"
	info "	- $RELEASE_DIR/$(basename "$WINDOWS_INSTALLER")"
	info "	- $RELEASE_DIR/$(basename "$WINDOWS_UPDATER")"
	info "	- $RELEASE_DIR/$(basename "$WINDOWS_UPDATER_SIG")"
else
	error "unsupported os: $OS"
fi

info "done! bye!"
