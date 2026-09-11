#!/bin/sh
set -eu

: "${PERF_ROOT:?PERF_ROOT is not set}"
# shellcheck disable=SC1091
. "$PERF_ROOT/lib.sh"

# Same real formatting commit as diff-many-uncommitted-changes: 1,167 files,
# 21,636 insertions and 21,620 deletions, left committed for this workload.
REAL_CHANGE_COMMIT=c9d8e3a7ff59f2ddabed16a6fa1d66ea054f0215
REAL_CHANGE_PARENT=$(
    "$GIT_BIN" --git-dir="$PERF_FIXTURE_REPO" rev-parse "$REAL_CHANGE_COMMIT^"
)

perf_reset_run_root
perf_create_gitbutler_workspace "$PERF_REPO" "$REAL_CHANGE_PARENT"

perf_git branch performance-diff "$REAL_CHANGE_COMMIT"
perf_but apply performance-diff >/dev/null
applied_commit=$(perf_git rev-parse refs/heads/performance-diff)
[ "$applied_commit" = "$REAL_CHANGE_COMMIT" ] ||
    perf_die "applying real change rewrote commit unexpectedly: $applied_commit"

[ -z "$(perf_git status --porcelain=v1)" ] ||
    perf_die "expected clean worktree for committed diff"

changed_files=$(perf_git diff-tree --no-commit-id --name-only -r "$applied_commit" | wc -l | tr -d ' ')
[ "$changed_files" -ge 1100 ] ||
    perf_die "expected at least 1100 committed paths, found $changed_files"

changed_lines=$(
    perf_git diff-tree --no-commit-id --numstat -r "$applied_commit" |
        awk '$1 != "-" && $2 != "-" { total += $1 + $2 } END { print total + 0 }'
)
[ "$changed_lines" -ge 40000 ] ||
    perf_die "expected at least 40000 changed lines, found $changed_lines"

perf_state_begin
perf_state_set DIFF_COMMIT "$applied_commit"
perf_state_commit
