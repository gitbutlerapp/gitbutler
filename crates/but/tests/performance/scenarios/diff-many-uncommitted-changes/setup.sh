#!/bin/sh
set -eu

: "${PERF_ROOT:?PERF_ROOT is not set}"
# shellcheck disable=SC1091
. "$PERF_ROOT/lib.sh"

# Real GitButler commit which formatted the codebase. It changes 1,167 files
# with 21,636 insertions and 21,620 deletions.
REAL_CHANGE_COMMIT=c9d8e3a7ff59f2ddabed16a6fa1d66ea054f0215
REAL_CHANGE_PARENT=$(
    "$GIT_BIN" --git-dir="$PERF_FIXTURE_REPO" rev-parse "$REAL_CHANGE_COMMIT^"
)

perf_reset_run_root
perf_create_gitbutler_workspace "$PERF_REPO" "$REAL_CHANGE_PARENT"

# Apply exact historical commit as a GitButler branch, then uncommit it. This
# leaves its real repository-wide changes in uncommitted area for timed diff.
perf_git branch performance-diff "$REAL_CHANGE_COMMIT"
perf_but apply performance-diff >/dev/null
applied_commit=$(perf_git rev-parse refs/heads/performance-diff)
[ "$applied_commit" = "$REAL_CHANGE_COMMIT" ] ||
    perf_die "applying real change rewrote commit unexpectedly: $applied_commit"
perf_but uncommit "$applied_commit" >/dev/null

changed_files=$(perf_git status --porcelain=v1 | wc -l | tr -d ' ')
[ "$changed_files" -ge 1100 ] ||
    perf_die "expected at least 1100 uncommitted paths, found $changed_files"

# Ensure setup produced substantial content changes, not merely many empty or
# metadata-only paths. Binary entries are ignored by this lower-bound check.
changed_lines=$(
    perf_git diff HEAD --numstat |
        awk '$1 != "-" && $2 != "-" { total += $1 + $2 } END { print total + 0 }'
)
[ "$changed_lines" -ge 40000 ] ||
    perf_die "expected at least 40000 changed lines, found $changed_lines"
