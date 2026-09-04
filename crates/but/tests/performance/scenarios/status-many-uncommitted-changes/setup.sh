#!/bin/sh
set -eu

: "${PERF_ROOT:?PERF_ROOT is not set}"
# shellcheck disable=SC1091
. "$PERF_ROOT/lib.sh"

# Real GitButler commit which replaced insta snapshots with snapbox. It changes
# 92 files with 40,185 insertions and 30,208 deletions.
REAL_CHANGE_COMMIT=702092d61c92c8d2093fc853b038c6f26b28207c
REAL_CHANGE_PARENT=$(
    "$GIT_BIN" --git-dir="$PERF_FIXTURE_REPO" rev-parse "$REAL_CHANGE_COMMIT^"
)

perf_reset_run_root
perf_create_gitbutler_workspace "$PERF_REPO" "$REAL_CHANGE_PARENT"

# Apply exact historical commit as a GitButler branch, then uncommit it. This
# leaves its real repository-wide changes in uncommitted area for timed status.
perf_git branch performance-status "$REAL_CHANGE_COMMIT"
perf_but apply performance-status >/dev/null
applied_commit=$(perf_git rev-parse refs/heads/performance-status)
[ "$applied_commit" = "$REAL_CHANGE_COMMIT" ] ||
    perf_die "applying real change rewrote commit unexpectedly: $applied_commit"
perf_but uncommit "$applied_commit" >/dev/null

changed_files=$(perf_git status --porcelain=v1 | wc -l | tr -d ' ')
[ "$changed_files" -ge 90 ] ||
    perf_die "expected at least 90 uncommitted paths, found $changed_files"

# Ensure setup produced substantial content changes, not merely many empty or
# metadata-only paths. Binary entries are ignored by this lower-bound check.
changed_lines=$(
    perf_git diff HEAD --numstat |
        awk '$1 != "-" && $2 != "-" { total += $1 + $2 } END { print total + 0 }'
)
[ "$changed_lines" -ge 70000 ] ||
    perf_die "expected at least 70000 changed lines, found $changed_lines"
