#!/bin/sh
set -eu

: "${PERF_ROOT:?PERF_ROOT is not set}"
# shellcheck disable=SC1091
. "$PERF_ROOT/lib.sh"
perf_use_run_environment
perf_state_load
: "${DIFF_COMMIT:?missing DIFF_COMMIT}"

perf_exec_but diff "$DIFF_COMMIT"
