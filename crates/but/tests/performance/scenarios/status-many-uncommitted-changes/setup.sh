#!/bin/sh
set -eu

: "${PERF_ROOT:?PERF_ROOT is not set}"
# shellcheck disable=SC1091
. "$PERF_ROOT/lib.sh"

perf_reset_run_root
perf_create_gitbutler_workspace "$PERF_REPO"

# Exercise automatic text conversion, which asks the object database for the
# index version while preparing each worktree-side diff resource.
perf_git config core.autocrlf input

# Create bogus changes
changed_paths=$PERF_RUN_ROOT/changed-paths
perf_git ls-files |
    awk '/\.rs$/ { print; if (++count == 240) exit }' >"$changed_paths"

selected_files=$(wc -l <"$changed_paths" | tr -d ' ')
[ "$selected_files" -eq 240 ] ||
    perf_die "expected to select 240 tracked Rust files, found $selected_files"

change_number=1
while IFS= read -r path; do
    printf '\n// performance status change %s\n' "$change_number" >>"$PERF_REPO/$path"
    change_number=$((change_number + 1))
done <"$changed_paths"

changed_files=$(perf_git status --porcelain=v1 | wc -l | tr -d ' ')
[ "$changed_files" -eq 240 ] ||
    perf_die "expected exactly 240 uncommitted paths, found $changed_files"
