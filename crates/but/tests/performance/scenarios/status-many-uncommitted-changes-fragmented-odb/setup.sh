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

# Reproduce a fragmented object database without modifying the shared fixture.
# A guaranteed object miss makes gix rescan every local pack index, so many
# small packs amplify the cost paid once per changed file.
#
# This is required to reproduce the terrible performance fixed by https://github.com/gitbutlerapp/gitbutler/pull/15746
git_dir=$(perf_git rev-parse --absolute-git-dir)
pack_dir=$git_dir/objects/pack
pack_number=1
while [ "$pack_number" -le 200 ]; do
    object_id=$(
        printf 'performance pack %s\n' "$pack_number" |
            perf_git hash-object -w --stdin
    )
    printf '%s\n' "$object_id" |
        perf_git pack-objects "$pack_dir/pack" >/dev/null 2>&1
    pack_number=$((pack_number + 1))
done

pack_count=$(find "$pack_dir" -type f -name '*.idx' | wc -l | tr -d ' ')
[ "$pack_count" -ge 200 ] ||
    perf_die "expected at least 200 local packs, found $pack_count"

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
