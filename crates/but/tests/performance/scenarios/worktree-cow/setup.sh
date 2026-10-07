#!/bin/sh
set -eu

: "${PERF_ROOT:?PERF_ROOT is not set}"
# shellcheck disable=SC1091
. "$PERF_ROOT/lib.sh"

# Fail before copying a potentially very large target directory.
[ "$(uname -s)" = Darwin ] ||
    perf_die 'worktree-cow requires macOS; Linux COW development mocks measure full copies'
: "${PERF_SOURCE_REPO:?PERF_SOURCE_REPO is not set}"
[ -d "$PERF_SOURCE_REPO/target/debug/deps" ] ||
    perf_die 'worktree-cow requires an existing target/debug/deps in the source repository'
source_target=$(CDPATH='' cd "$PERF_SOURCE_REPO/target" && pwd -P)
case "$PERF_SESSION_ROOT/" in
    "$source_target/"*) perf_die 'TMPDIR must be outside the source target directory' ;;
esac
help=$("$BUT_BIN" worktree new --help)
case "$help" in
    *--create-mode*cow*) ;;
    *) perf_die 'worktree-cow requires a but binary built with COW support (--create-mode cow)' ;;
esac

perf_reset_run_root
settings=$E2E_TEST_APP_DATA_DIR/gitbutler/settings.json
jq '.featureFlags.worktreeManipulation = true' "$settings" >"$settings.tmp"
mv "$settings.tmp" "$settings"
perf_create_gitbutler_workspace "$PERF_REPO"

# Copy actual local build state, not synthetic artifacts. -c requests APFS clones;
# -p preserves timestamps/permissions and -R preserves the directory structure.
# Source and session must share a filesystem supporting COW. No hard links.
# Keep the source target idle throughout the benchmark.
cp -cRp "$source_target" "$PERF_REPO/target"
perf_git check-ignore -q target/debug/deps ||
    perf_die 'fixture must ignore target/debug/deps so checkout preserves cloned artifacts'
