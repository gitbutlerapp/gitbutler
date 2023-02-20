#!/bin/bash

set -o errexit
set -o nounset
set -o pipefail

PWD="$(dirname $(readlink -f -- $0))"

DO_SIGN="false"
DO_BUNDLE_UPDATE="false"
TAURI_PRIVATE_KEY=""
TAURI_KEY_PASSWORD=""
APPLE_CERTIFICATE=""
APPLE_CERTIFICATE_PASSWORD=""
APPLE_SIGNING_IDENTITY=""
APPLE_ID=""
APPLE_PASSWORD=""
VERSION=""

function help() {
	local to="$1"

	echo "Usage: $0 <flags>" 1>&$to
	echo 1>&$to
	echo "flags:" 1>&$to
	echo "  --version                     release version." 1>&$to
	echo "  --tauri-private-key           path or string of tauri updater private key." 1>&$to
	echo "  --tauri-key-password          password for tauri updater private key." 1>&$to
	echo "  --apple-certificate           base64 string of the .p12 certificate, exported from the keychain." 1>&$to
	echo "  --apple-certificate-password  password for the .p12 certificate." 1>&$to
	echo "  --apple-signing-identity      the name of the keychain entry that contains the signing certificate." 1>&$to
	echo "  --apple-id                    the apple id to use for signing." 1>&$to
	echo "  --apple-password              the password for the apple id." 1>&$to
	echo "  --sign                        if set, will sign the app." 1>&$to
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
	*)
		error "$os: unsupprted"
		;;
	esac
}

function arch() {
	local arch="$(uname -m)"
	case "$arch" in
	arm64)
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
	pushd "$PWD/.." >/dev/null
	pnpm tauri "$@"
	popd >/dev/null
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
	--apple-password)
		APPLE_PASSWORD="$2"
		shift
		shift
		;;
	--sign)
		DO_SIGN="true"
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

export TAURI_PRIVATE_KEY="$TAURI_PRIVATE_KEY"
export TAURI_KEY_PASSWORD="$TAURI_KEY_PASSWORD"

if [ "$DO_SIGN" = "true" ]; then
	[ -z "$APPLE_CERTIFICATE" ] && error "--apple-certificate is not set"
	[ -z "$APPLE_CERTIFICATE_PASSWORD" ] && error "--apple-certificate-password is not set"
	[ -z "$APPLE_SIGNING_IDENTITY" ] && error "--apple-signing-identity is not set"
	[ -z "$APPLE_ID" ] && error "--apple-id is not set"
	[ -z "$APPLE_PASSWORD" ] && error "--apple-password is not set"

	export APPLE_CERTIFICATE="$APPLE_CERTIFICATE"
	export APPLE_CERTIFICATE_PASSWORD="$APPLE_CERTIFICATE_PASSWORD"
	export APPLE_SIGNING_IDENTITY="$APPLE_SIGNING_IDENTITY"
	export APPLE_ID="$APPLE_ID"
	export APPLE_PASSWORD="$APPLE_PASSWORD"
fi

info "building:"
info "  version: $VERSION"
info "  os: $OS"
info "  arch: $ARCH"
info "  dist: $DIST"

TMP_DIR="$(mktemp -d)"
trap "rm -rf '$TMP_DIR'" exit

# update the version in the tauri config
jq '.package.version="'"$VERSION"'"' "$PWD/../src-tauri/tauri.conf.json" >"$TMP_DIR/tauri.conf.json"

# build the app
tauri build --config "$TMP_DIR/tauri.conf.json"

BUNDLE_DIR="src-tauri/target/release/bundle"
MACOS_DMG="$(find "$BUNDLE_DIR/dmg" -depth 1 -type f -name "*.dmg")"
MACOS_UPDATER="$(find "$BUNDLE_DIR/macos" -depth 1 -type f -name "*.tar.gz")"
MACOS_UPDATER_SIG="$(find "$BUNDLE_DIR/macos" -depth 1 -type f -name "*.tar.gz.sig")"

RELEASE_DIR="$DIST/$OS/$ARCH"
cp "$MACOS_DMG" "$RELEASE_DIR"
cp "$MACOS_UPDATER" "$RELEASE_DIR"
cp "$MACOS_UPDATER_SIG" "$RELEASE_DIR"

info "built:"
info "  - $RELEASE_DIR/$(basename "$MACOS_DMG")"
info "  - $RELEASE_DIR/$(basename "$MACOS_UPDATER")"
info "  - $RELEASE_DIR/$(basename "$MACOS_UPDATER_SIG")"

info "done! bye!"
