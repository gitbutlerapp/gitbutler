#!/bin/bash

set -o errexit
set -o pipefail

PWD="$(dirname $(readlink -f -- $0))"
DIST="$PWD/git"
VERSION="2.39.1"

function help() {
	echo "Usage: $0 <flags>"
	echo
	echo "flags:"
	echo "  --version  git version to install.       (default: $VERSION)"
	echo "  --dist     directory to install binaries to. (default: $DIST)"
	echo "  --help     display this message."
}

function error() {
	echo "error: $@"
	echo
	help
	exit 1
}

function info() {
	echo "$@"
}

while [[ $# -gt 0 ]]; do
	case "$1" in
	--help)
		help
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
	*)
		error "unknown flag $1"
		;;
	esac
done

function install_dependencies() {
	case "$(uname -s)" in
	Darwin)
		if [[ -z "$(brew ls --versions gettext)" ]]; then
			brew install gettext
		fi

		if [[ -z "$(brew ls --versions automake)" ]]; then
			brew install automake
		fi
		;;
	esac
}

function configure() {
	case "$(uname -s)" in
	Darwin)
		make configure
		./configure --prefix=/opt/homebrew
		;;
	esac
}

TMP_DIR=$(mktemp -d -t ci-XXXXXXXXXX)
pushd "$TMP_DIR"

install_dependencies

curl --location --remote-name "https://www.kernel.org/pub/software/scm/git/git-$VERSION.tar.gz"
tar -xzf "git-$VERSION.tar.gz"
cd "git-$VERSION"

configure
make

mv 'git' "$DIST"

popd
rm -rf $TMP_DIR
