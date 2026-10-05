#!/bin/sh
set -eu

: "${PERF_ROOT:?PERF_ROOT is not set}"
# shellcheck disable=SC1091
. "$PERF_ROOT/lib.sh"

perf_reset_run_root
perf_create_gitbutler_workspace "$PERF_REPO"
perf_but branch new performance >/dev/null

commit=1
while [ "$commit" -le 8 ]; do
    benchmark_file=".gitbutler-performance/absorb-$commit.txt"
    mkdir -p "$PERF_REPO/$(dirname "$benchmark_file")"
    line=1
    : >"$PERF_REPO/$benchmark_file"
    while [ "$line" -le 200 ]; do
        printf 'commit %02d original line %03d\n' "$commit" "$line" >>"$PERF_REPO/$benchmark_file"
        line=$((line + 1))
    done
    perf_but commit -b performance -m "performance target $commit" "$benchmark_file" >/dev/null
    commit=$((commit + 1))
done

commit=1
while [ "$commit" -le 8 ]; do
    benchmark_file=".gitbutler-performance/absorb-$commit.txt"
    line=1
    : >"$PERF_REPO/$benchmark_file"
    while [ "$line" -le 200 ]; do
        case "$line" in
            20|60|100|140)
                printf 'commit %02d modified line %03d\n' "$commit" "$line" >>"$PERF_REPO/$benchmark_file"
                ;;
            *)
                printf 'commit %02d original line %03d\n' "$commit" "$line" >>"$PERF_REPO/$benchmark_file"
                ;;
        esac
        line=$((line + 1))
    done
    commit=$((commit + 1))
done

plan=$PERF_RUN_ROOT/absorb-plan.json
perf_but --json absorb --dry-run >"$plan"
jq -e '
    .total_files == 8 and
    (.commits | length) == 8 and
    ([.commits[].files[].hunks[]] | length) == 32
' "$plan" >/dev/null || {
    cat "$plan" >&2
    perf_die "expected 8 targets and 32 absorb hunks"
}

modified_count=$(perf_git status --porcelain | awk '$1 == "M" { count += 1 } END { print count + 0 }')
[ "$modified_count" = 8 ] || perf_die "expected 8 modified paths, found $modified_count"