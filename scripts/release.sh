#!/bin/bash

set -o errexit
set -o nounset
set -o pipefail

PWD="$(dirname $(readlink -f -- $0))"

CHANNEL=""
DO_SIGN="false"
DO_BUNDLE_UPDATE="false"
TAURI_PRIVATE_KEY=""
TAURI_KEY_PASSWORD=""
APPLE_CERTIFICATE=""
APPLE_CERTIFICATE_PASSWORD=""
APPLE_SIGNING_IDENTITY=""
APPLE_ID=""
APPLE_PASSWORD=""
APPIMAGE_KEY_ID=""
APPIMAGE_KEY_PASSPHRASE=""
VERSION=""
SENTRY_AUTH_TOKEN="$SENTRY_AUTH_TOKEN"

function help() {
	local to="$1"

	echo "Usage: $0 <flags>" 1>&$to
	echo 1>&$to
	echo "flags:" 1>&$to
	echo "  --version                     release version." 1>&$to
	echo "  --dist                        path to store artifacts in." 1>&$to
	echo "  --tauri-private-key           path or string of tauri updater private key." 1>&$to
	echo "  --tauri-key-password          password for tauri updater private key." 1>&$to
	echo "  --apple-certificate           base64 string of the .p12 certificate, exported from the keychain." 1>&$to
	echo "  --apple-certificate-password  password for the .p12 certificate." 1>&$to
	echo "  --apple-signing-identity      the name of the keychain entry that contains the signing certificate." 1>&$to
	echo "  --apple-id                    the apple id to use for signing." 1>&$to
	echo "  --apple-team-id               the apple team id to use for signing." 1>&$to
	echo "  --apple-password              the password for the apple id." 1>&$to
	echo "  --appimage-key-id             the gpg key id to use for signing the appimage." 1>&$to
	echo "  --appimage-key-passphrase     the gpg key passphrase to use for signing the appimage." 1>&$to
	echo "  --sign                        if set, will sign the app." 1>&$to
	echo "  --channel                     the channel to use for the release (release | nightly)." 1>&$to
	echo "  --help                        display this message." 1>&$to
}

function error() {
	echo "error: $@" 1>&2
	echo 1>&2
	help 2
	exit 1
}

function info() {
	echo "$@"
}

function os() {
	local os="$(uname -s)"
	case "$os" in
	Darwin)
		echo "macos"
		;;
	Linux)
		echo "linux"
		;;
	*)
		error "$os: unsupprted"
		;;
	esac
}

function arch() {
	local arch="$(uname -m)"
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
	(cd "$PWD/.." && pnpm tauri "$@")
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
	--tauri-private-key)
		TAURI_PRIVATE_KEY="$2"
		shift
		shift
		;;
	--tauri-key-password)
		TAURI_KEY_PASSWORD="$2"
		shift
		shift
		;;
	--apple-certificate)
		APPLE_CERTIFICATE="$2"
		shift
		shift
		;;
	--apple-certificate-password)
		APPLE_CERTIFICATE_PASSWORD="$2"
		shift
		shift
		;;
	--apple-signing-identity)
		APPLE_SIGNING_IDENTITY="$2"
		shift
		shift
		;;
	--apple-id)
		APPLE_ID="$2"
		shift
		shift
		;;
	--apple-team-id)
		APPLE_TEAM_ID="$2"
		shift
		shift
		;;
	--apple-password)
		APPLE_PASSWORD="$2"
		shift
		shift
		;;
	--appimage-key-id)
		APPIMAGE_KEY_ID="$2"
		shift
		shift
		;;
	--appimage-key-passphrase)
		APPIMAGE_KEY_PASSPHRASE="$2"
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

[ -z "$VERSION" ] && error "--version is not set"

[ -z "$TAURI_PRIVATE_KEY" ] && error "--tauri-private-key is not set"
[ -z "$TAURI_KEY_PASSWORD" ] && error "--tauri-key-password is not set"

if [ "$CHANNEL" != "release" ] && [ "$CHANNEL" != "nightly" ]; then
	error "--channel must be either 'release' or 'nightly'"
fi

export TAURI_PRIVATE_KEY="$TAURI_PRIVATE_KEY"
export TAURI_KEY_PASSWORD="$TAURI_KEY_PASSWORD"

if [ "$DO_SIGN" = "true" ]; then
	if [ "$OS" = "macos" ]; then
		[ -z "$APPLE_CERTIFICATE" ] && error "--apple-certificate is not set"
		[ -z "$APPLE_CERTIFICATE_PASSWORD" ] && error "--apple-certificate-password is not set"
		[ -z "$APPLE_SIGNING_IDENTITY" ] && error "--apple-signing-identity is not set"
		[ -z "$APPLE_ID" ] && error "--apple-id is not set"
		[ -z "$APPLE_TEAM_ID" ] && error "--apple-team-id is not set"
		[ -z "$APPLE_PASSWORD" ] && error "--apple-password is not set"
		export APPLE_CERTIFICATE="$APPLE_CERTIFICATE"
		export APPLE_CERTIFICATE_PASSWORD="$APPLE_CERTIFICATE_PASSWORD"
		export APPLE_SIGNING_IDENTITY="$APPLE_SIGNING_IDENTITY"
		export APPLE_ID="$APPLE_ID"
		export APPLE_TEAM_ID="$APPLE_TEAM_ID"
		export APPLE_PASSWORD="$APPLE_PASSWORD"
	elif [ "$OS" == "linux" ]; then
		[ -z "$APPIMAGE_KEY_ID" ] && error "--appimage-key-id is not set"
		[ -z "$APPIMAGE_KEY_PASSPHRASE" ] && error "--appimage-key-passphrase is not set"
		export SIGN=1
		export SIGN_KEY="$APPIMAGE_KEY_ID"
		export APPIMAGETOOL_SIGN_PASSPHRASE="$APPIMAGE_KEY_PASSPHRASE"
	else
		error "signing is not supported on $(uname -s)"
	fi
fi

info "building:"
info "  channel: $CHANNEL"
info "  version: $VERSION"
info "  os: $OS"
info "  arch: $ARCH"
info "  dist: $DIST"

TMP_DIR="$(mktemp -d)"
trap "rm -rf '$TMP_DIR'" exit

CONFIG_PATH=$(readlink -f "$PWD/../gitbutler-app/tauri.conf.$CHANNEL.json")

# update the version in the tauri release config
jq '.package.version="'"$VERSION"'"' "$CONFIG_PATH" >"$TMP_DIR/tauri.conf.json"

FEATURES=""

if [ "$CHANNEL" == "nightly" ]; then
	FEATURES="$FEATURES devtools"
fi

# build the app with release config
SENTRY_RELEASE="$VERSION" tauri build \
	--verbose \
	--features "$FEATURES" \
	--config "$TMP_DIR/tauri.conf.json"

BUNDLE_DIR=$(readlink -f "$PWD/../target/release/bundle")
RELEASE_DIR="$DIST/$OS/$ARCH"
mkdir -p "$RELEASE_DIR"

if [ "$OS" = "macos" ]; then
	MACOS_DMG="$(find "$BUNDLE_DIR/dmg" -depth 1 -type f -name "*.dmg")"
	MACOS_UPDATER="$(find "$BUNDLE_DIR/macos" -depth 1 -type f -name "*.tar.gz")"
	MACOS_UPDATER_SIG="$(find "$BUNDLE_DIR/macos" -depth 1 -type f -name "*.tar.gz.sig")"

	cp "$MACOS_DMG" "$RELEASE_DIR"
	cp "$MACOS_UPDATER" "$RELEASE_DIR"
	cp "$MACOS_UPDATER_SIG" "$RELEASE_DIR"

	info "built:"
	info "  - $RELEASE_DIR/$(basename "$MACOS_DMG")"
	info "  - $RELEASE_DIR/$(basename "$MACOS_UPDATER")"
	info "  - $RELEASE_DIR/$(basename "$MACOS_UPDATER_SIG")"
elif [ "$OS" = "linux" ]; then
	APPIMAGE="$(find $BUNDLE_DIR/appimage -name \*.AppImage)"
	APPIMAGE_UPDATER="$(find $BUNDLE_DIR/appimage -name \*.AppImage.tar.gz)"
	APPDIR="$(find $BUNDLE_DIR/appimage -name \*.AppDir)"
	APPIMAGE_UPDATER_SIG="$(find $BUNDLE_DIR/appimage -name \*.AppImage.tar.gz.sig)"
	DEB="$(find $BUNDLE_DIR/deb -name \*.deb)"

    tar --exclude=usr/bin/xdg-open --exclude=usr/lib --exclude=usr/share/doc --exclude=usr/share/glib-2.0 -zcvf "$BUNDLE_DIR/gitbutler.tar.gz" -C "$APPDIR" usr
    TAR=$(find $BUNDLE_DIR -name gitbutler.tar.gz)

	cp "$APPIMAGE" "$RELEASE_DIR"
	cp "$APPIMAGE_UPDATER" "$RELEASE_DIR"
	cp "$APPIMAGE_UPDATER_SIG" "$RELEASE_DIR"
	cp "$DEB" "$RELEASE_DIR"
    cp "$TAR" "$RELEASE_DIR"

	info "built:"
	info "  - $RELEASE_DIR/$(basename "$APPIMAGE")"
	info "  - $RELEASE_DIR/$(basename "$APPIMAGE_UPDATER")"
	info "  - $RELEASE_DIR/$(basename "$APPIMAGE_UPDATER_SIG")"
	info "  - $RELEASE_DIR/$(basename "$DEB")"
    info "  - $RELEASE_DIR/$(basename "$TAR")"
fi

info "done! bye!"
