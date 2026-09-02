#!/bin/sh
set -eu

: "${PERF_ROOT:?PERF_ROOT is not set}"
# shellcheck disable=SC1091
. "$PERF_ROOT/lib.sh"

perf_reset_run_root
perf_create_gitbutler_workspace "$PERF_REPO"

benchmark_file=.gitbutler-performance/ten-hunks.txt
mkdir -p "$PERF_REPO/$(dirname "$benchmark_file")"

line=1
: >"$PERF_REPO/$benchmark_file"
while [ "$line" -le 200 ]; do
    printf 'original line %03d\n' "$line" >>"$PERF_REPO/$benchmark_file"
    line=$((line + 1))
done

perf_but branch new performance >/dev/null
perf_but commit -b performance -m 'performance target' "$benchmark_file" >/dev/null

line=1
: >"$PERF_REPO/$benchmark_file"
while [ "$line" -le 200 ]; do
    case "$line" in
        10|30|50|70|90|110|130|150|170|190)
            printf 'modified line %03d\n' "$line" >>"$PERF_REPO/$benchmark_file"
            ;;
        *)
            printf 'original line %03d\n' "$line" >>"$PERF_REPO/$benchmark_file"
            ;;
    esac
    line=$((line + 1))
done

perf_but commit -b performance -m 'performance source' "$benchmark_file" >/dev/null

source_commit=$(perf_git rev-parse refs/heads/performance)
target_commit=$(perf_git rev-parse "$source_commit^")
diff_output=$PERF_RUN_ROOT/source-diff.txt
if ! perf_but diff "$source_commit" >"$diff_output"; then
    perf_but status >&2
    perf_die "could not resolve source commit: $source_commit"
fi
hunk_ids=$PERF_RUN_ROOT/hunk-ids
awk -v path="$benchmark_file" 'NF >= 2 && $(NF - 1) == path && $NF == "│" { print $1 }' \
    "$diff_output" >"$hunk_ids"

hunk_count=$(wc -l <"$hunk_ids" | tr -d ' ')
[ "$hunk_count" = 10 ] || {
    cat "$diff_output" >&2
    perf_die "expected 10 committed hunks, found $hunk_count"
}

perf_state_begin
perf_state_set TARGET_COMMIT "$target_commit"
index=1
while [ "$index" -le 10 ]; do
    hunk_id=$(sed -n "${index}p" "$hunk_ids")
    [ -n "$hunk_id" ] || perf_die "missing hunk ID $index"
    perf_state_set "HUNK_$index" "$hunk_id"
    index=$((index + 1))
done
perf_state_commit
