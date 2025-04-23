#!/bin/bash

set -o errexit
set -o nounset
set -o pipefail

function help() {
	local to="$1"

	echo "Usage: $0 <version> <bump>" 1>&$to
	echo 1>&$to
	echo "where:" 1>&$to
	echo "  <version> is a current semver version." 1>&$to
	echo "  <bump> is either 'patch', 'major' or 'minor'." 1>&$to
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

VERSION="$1"
BUMP="$2"

# https://semver.org/
SEMVER_REGEX="^(0|[1-9][[:digit:]]*)\.(0|[1-9][[:digit:]]*)\.(0|[1-9][[:digit:]]*)(?:-((?:0|[1-9][[:digit:]]*|[[:digit:]]*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9][[:digit:]]*|[[:digit:]]*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$"
(echo "$VERSION" | grep -Eq "$SEMVER_REGEX") || error "'$VERSION' not a semver"

case "$BUMP" in
major)
	MAJOR="$(($(echo "$VERSION" | cut -d. -f1) + 1))"
	MINOR="0"
	PATCH="0"
	echo "$MAJOR.$MINOR.$PATCH"
	;;
minor)
	MAJOR="$(echo "$VERSION" | cut -d. -f1)"
	MINOR="$(($(echo "$VERSION" | cut -d. -f2) + 1))"
	PATCH="0"
	echo "$MAJOR.$MINOR.$PATCH"
	;;
patch)
	MAJOR="$(echo "$VERSION" | cut -d. -f1)"
	MINOR="$(echo "$VERSION" | cut -d. -f2)"
	PATCH="$(($(echo "$VERSION" | cut -d. -f3) + 1))"
	echo "$MAJOR.$MINOR.$PATCH"
	;;
*)
	error "invalid bump type: $BUMP"
	;;
esac
