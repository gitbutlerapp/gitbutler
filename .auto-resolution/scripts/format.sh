#!/bin/bash

set -o errexit
set -o nounset
set -o pipefail

function rust() {
	cargo fmt
}

function node() {
	pnpm fix
	pnpm format
}

if [[ "$#" -eq 0 ]]; then
	set -o xtrace
	rust
	node
else
	case "$1" in
	rust)
		set -o xtrace
		rust
		;;
	node)
		set -o xtrace
		node
		;;
	*)
		echo "Invalid argument: $1"
		exit 1
		;;
	esac
	exit 0
fi
