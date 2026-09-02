#!/bin/sh
set -eu

: "${PERF_ROOT:?PERF_ROOT is not set}"
# shellcheck disable=SC1091
. "$PERF_ROOT/lib.sh"
perf_use_run_environment
perf_state_load

: "${TARGET_COMMIT:?missing TARGET_COMMIT}"
: "${HUNK_1:?missing HUNK_1}"
: "${HUNK_2:?missing HUNK_2}"
: "${HUNK_3:?missing HUNK_3}"
: "${HUNK_4:?missing HUNK_4}"
: "${HUNK_5:?missing HUNK_5}"
: "${HUNK_6:?missing HUNK_6}"
: "${HUNK_7:?missing HUNK_7}"
: "${HUNK_8:?missing HUNK_8}"
: "${HUNK_9:?missing HUNK_9}"
: "${HUNK_10:?missing HUNK_10}"

exec "$BUT_BIN" -C "$PERF_REPO" squash \
    "$HUNK_1" \
    "$HUNK_2" \
    "$HUNK_3" \
    "$HUNK_4" \
    "$HUNK_5" \
    "$HUNK_6" \
    "$HUNK_7" \
    "$HUNK_8" \
    "$HUNK_9" \
    "$HUNK_10" \
    --target "$TARGET_COMMIT" \
    --use-target-message \
    >/dev/null
