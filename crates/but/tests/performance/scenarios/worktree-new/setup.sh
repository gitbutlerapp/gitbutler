#!/bin/sh
set -eu

: "${PERF_ROOT:?PERF_ROOT is not set}"
# shellcheck disable=SC1091
. "$PERF_ROOT/lib.sh"

perf_reset_run_root

settings=$E2E_TEST_APP_DATA_DIR/gitbutler/settings.json
jq '.featureFlags.worktreeManipulation = true' "$settings" >"$settings.tmp"
mv "$settings.tmp" "$settings"

perf_create_gitbutler_workspace "$PERF_REPO"
