#!/bin/sh
set -eu

: "${PERF_ROOT:?PERF_ROOT is not set}"
# shellcheck disable=SC1091
. "$PERF_ROOT/lib.sh"

perf_require_command script
perf_require_command stty
platform=$(uname -s)
case "$platform" in
    Linux|Darwin) ;;
    *) perf_die "unsupported PTY platform: $platform" ;;
esac

perf_reset_run_root
perf_create_gitbutler_workspace "$PERF_REPO"
perf_state_begin
perf_state_set PTY_PLATFORM "$platform"
perf_state_commit
