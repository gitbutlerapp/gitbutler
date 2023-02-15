#!/bin/bash

set -o errexit
set -o nounset
set -o pipefail

PWD="$(dirname $(readlink -f -- $0))"

DO_UPLOAD="false"
DO_SIGN="false"
DO_BUNDLE_UPDATE="false"
TAURI_PRIVATE_KEY=""
TAURI_KEY_PASSWORD=""
APPLE_CERTIFICATE=""
APPLE_CERTIFICATE_PASSWORD=""
APPLE_SIGNING_IDENTITY=""
APPLE_ID=""
APPLE_PASSWORD=""

function help() {
	local to="$1"

	echo "Usage: $0 <flags>" 1>&$to
	echo 1>&$to
	echo "flags:" 1>&$to
	echo "  --tauri-private-key           path or string of tauri updater private key." 1>&$to
	echo "  --tauri-key-password          password for tauri updater private key." 1>&$to
	echo "  --apple-certificate           base64 string of the .p12 certificate, exported from the keychain." 1>&$to
	echo "  --apple-certificate-password  password for the .p12 certificate." 1>&$to
	echo "  --apple-signing-identity      the name of the keychain entry that contains the signing certificate." 1>&$to
	echo "  --apple-id                    the apple id to use for signing." 1>&$to
	echo "  --apple-password              the password for the apple id." 1>&$to
	echo "  --sign                        if set, will sign the app." 1>&$to
	echo "  --upload                      if set, will upload artifacts to S3." 1>&$to
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

function tauri() {
	pushd "$PWD/.."
	pnpm tauri "$@"
	popd
}

while [[ $# -gt 0 ]]; do
	case "$1" in
	--help)
		help 1
		exit 1
		;;
	--upload)
		DO_UPLOAD="true"
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

tauri build --config "$PWD/../src-tauri/tauri.conf.release.json"
