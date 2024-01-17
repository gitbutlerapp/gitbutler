#!/bin/bash

set -o errexit
set -o nounset
set -o pipefail

function rust() {
	cargo fmt --check
	cargo clippy --all-targets --all-features --tests
	cargo test
}

function node() {
	pnpm lint
	pnpm check
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
