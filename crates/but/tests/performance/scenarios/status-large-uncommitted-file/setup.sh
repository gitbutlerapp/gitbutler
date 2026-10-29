#!/bin/sh
set -eu

: "${PERF_ROOT:?PERF_ROOT is not set}"
# shellcheck disable=SC1091
. "$PERF_ROOT/lib.sh"

perf_reset_run_root
perf_create_gitbutler_workspace "$PERF_REPO"

dd if=/dev/urandom of="$PERF_REPO/large-file.bin" bs=1048576 count=400
