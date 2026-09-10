#!/bin/sh
set -eu

PERF_ROOT=$(CDPATH='' cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(CDPATH='' cd "$PERF_ROOT/../../../.." && pwd)
# shellcheck disable=SC1091
. "$PERF_ROOT/lib.sh"

# Only perf_exec_but enables this mode, after scenario state/argv are resolved.
if [ "${PERF_ENV_ISOLATED:-}" = 1 ] && [ "${PERF_PROFILE_EXEC:-}" = 1 ]; then
    : "${PERF_PROFILE_OUTPUT_DIR:?}"
    : "${PERF_PROFILER_BIN:?}"
    printf '%s\n' "$@" >"$PERF_PROFILE_OUTPUT_DIR/command.txt"
    cd "$PERF_PROFILE_OUTPUT_DIR"
    case "$PERF_PROFILE_BACKEND" in
        samply)
            set -- "$PERF_PROFILER_BIN" record --save-only -o "$PWD/profile.json" -- "$@"
            ;;
        perf)
            set -- "$PERF_PROFILER_BIN" record -o "$PWD/perf.data" --call-graph dwarf -- "$@"
            ;;
        flamegraph)
            if [ "${PERF_PROFILE_PREFLIGHT:-}" = 1 ]; then
                # A --version process may yield zero samples: test recording access,
                # not SVG conversion, which correctly rejects an empty profile.
                case "$(uname -s)" in
                    Linux) set -- perf record -o "$PWD/perf.data" --call-graph dwarf -- "$@" ;;
                    Darwin) set -- xcrun xctrace record --template 'Time Profiler' \
                        --output "$PWD/preflight.trace" --target-stdout - --launch -- "$@" ;;
                esac
            else
                set -- "$PERF_PROFILER_BIN" -o "$PWD/flamegraph.svg" -- "$@"
            fi
            ;;
        *) perf_die "unknown profiling backend: $PERF_PROFILE_BACKEND" ;;
    esac
    # No target wrapper: macOS samply must inject directly into locally built but.
    # Profiler diagnostics and target stderr are retained and printed by parent.
    if [ "$PERF_SHOW_OUTPUT" = 1 ]; then
        exec "$@" 2>"$PERF_PROFILE_OUTPUT_DIR/profiler.log"
    else
        exec "$@" >/dev/null 2>"$PERF_PROFILE_OUTPUT_DIR/profiler.log"
    fi
fi

[ "$#" -eq 2 ] || perf_die 'usage: profile.sh <samply|perf|flamegraph> <scenario>'
backend=$1
scenario_name=$2
case "$backend" in
    samply|perf|flamegraph) ;;
    *) perf_die "unknown profiling backend: $backend (expected samply, perf, or flamegraph)" ;;
esac
perf_validate_scenario "$scenario_name"
setup_script=$PERF_ROOT/scenarios/$scenario_name/setup.sh
test_script=$PERF_ROOT/scenarios/$scenario_name/test.sh

# Not every profiler forwards termination (flamegraph also launches a recorder).
# Stop descendants first, while parents can still reap them. Subshell keeps each
# recursive PID local; only this runner's active child tree is affected.
profile_stop() (
    for descendant in $(ps -e -o pid= -o ppid= | awk -v parent="$1" '$2 == parent { print $1 }'); do
        profile_stop "$descendant"
    done
    kill -TERM "$1" 2>/dev/null || true
)

# Wait explicitly so signals to entrypoint reach active setup/recording child before
# outer EXIT trap removes its fixture. TERM also stops shells launched asynchronously,
# which POSIX permits to inherit ignored SIGINT.
profile_wait() {
    "$@" &
    profile_child=$!
    trap 'trap "" INT HUP TERM; profile_stop "$profile_child"; wait "$profile_child" || true; exit 130' INT
    trap 'trap "" INT HUP TERM; profile_stop "$profile_child"; wait "$profile_child" || true; exit 143' HUP TERM
    profile_status=0
    wait "$profile_child" || profile_status=$?
    trap 'exit 130' INT
    trap 'exit 143' HUP TERM
    return "$profile_status"
}

if [ "${PERF_ENV_ISOLATED:-}" != 1 ]; then
    case "$(uname -s):$backend" in
        Linux:*|Darwin:samply|Darwin:flamegraph) ;;
        *) perf_die "$backend is not supported on $(uname -s); use samply on macOS" ;;
    esac
    case "${PERF_SHOW_OUTPUT:-0}" in
        0|1) ;;
        *) perf_die 'PERF_SHOW_OUTPUT must be 0 or 1' ;;
    esac
    [ -z "${PERF_CHANNEL:-}${PERF_VERSION:-}${PERF_UPLOAD_URL:-}${PERF_UPLOAD_TOKEN:-}${PERF_RESULTS_DIR:-}${PERF_WARMUP:-}${PERF_MIN_RUNS:-}${PERF_RUNS:-}" ] ||
        perf_die 'benchmark download/upload/timing settings are not supported; use BUT_BIN and PERF_PROFILE_OUTPUT_DIR'
    perf_require_command git
    GIT_BIN=$(command -v git)
    GIT_BIN=$(CDPATH='' cd "$(dirname "$GIT_BIN")" && pwd)/$(basename "$GIT_BIN")
    perf_require_command "$backend"
    PERF_PROFILER_BIN=$(command -v "$backend")
    # Make relative PATH entries safe across fixture/output directory changes.
    PERF_PROFILER_BIN=$(CDPATH='' cd "$(dirname "$PERF_PROFILER_BIN")" && pwd)/$(basename "$PERF_PROFILER_BIN")
    # Outer fixture checks and metadata run before perf_run_isolated.
    # Keep caller repository overrides from redirecting these Git commands.
    unset GIT_DIR GIT_INDEX_FILE GIT_OBJECT_DIRECTORY
    unset GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_WORK_TREE GIT_COMMON_DIR
    "$GIT_BIN" -C "$REPO_ROOT" cat-file -e "$PERF_FIXTURE_COMMIT^{commit}" ||
        perf_die "fixture commit missing from source checkout: $PERF_FIXTURE_COMMIT"
    if [ "$backend" = flamegraph ]; then
        case "$(uname -s)" in
            Linux) perf_require_command perf ;;
            Darwin)
                perf_require_command xcrun
                xcrun xctrace version || perf_die 'install/select full Xcode with Instruments, or use samply'
                xcrun xctrace list templates | grep -q 'Time Profiler' ||
                    perf_die 'Xcode Time Profiler template unavailable; use samply'
                ;;
        esac
    fi

    if [ -n "${PERF_PROFILE_OUTPUT_DIR:-}" ]; then
        # Require new directory: no partial overwrite of an earlier capture.
        mkdir -p "$(dirname "$PERF_PROFILE_OUTPUT_DIR")"
        mkdir "$PERF_PROFILE_OUTPUT_DIR" || perf_die 'profile output directory must not already exist'
    else
        output_parent=$REPO_ROOT/target/performance-profiles/$scenario_name
        mkdir -p "$output_parent"
        PERF_PROFILE_OUTPUT_DIR=$(mktemp -d "$output_parent/$backend.XXXXXX")
    fi
    PERF_PROFILE_OUTPUT_DIR=$(CDPATH='' cd "$PERF_PROFILE_OUTPUT_DIR" && pwd)
    printf 'Profile artifacts: %s\n' "$PERF_PROFILE_OUTPUT_DIR" >&2
    perf_create_session

    build_description='supplied binary; build settings/revision unknown'
    if [ -n "${BUT_BIN:-}" ]; then
        perf_resolve_binary
    else
        perf_require_command cargo
        perf_require_command rustc
        # Explicit native target/path avoids assumptions about Cargo target-dir or
        # configured cross-compilation targets; no JSON parser dependency needed.
        host=$(rustc -vV | awk '/^host:/ { print $2 }')
        [ -n "$host" ] || perf_die 'could not determine native Rust target'
        build_dir=$REPO_ROOT/target/profiling-build
        build_description="bench profile, debug=true, strip=none, native target=$host"
        printf 'Building optimized, symbolized but binary...\n' >&2
        (
            cd "$REPO_ROOT"
            export CARGO_PROFILE_BENCH_DEBUG=true CARGO_PROFILE_BENCH_STRIP=none
            if [ "$(uname -s)" = Darwin ]; then
                export CARGO_PROFILE_BENCH_SPLIT_DEBUGINFO=packed
            fi
            cargo build --profile bench -p but --bin but --target "$host" --target-dir "$build_dir"
        )
        BUT_BIN=$build_dir/$host/release/but
        perf_resolve_binary
    fi
    original_binary=$BUT_BIN
    # Record the retained executable itself so viewers resolve symbols even after
    # another build replaces target/profiling-build. Keep adjacent dSYM on macOS.
    mkdir "$PERF_PROFILE_OUTPUT_DIR/binary"
    cp "$BUT_BIN" "$PERF_PROFILE_OUTPUT_DIR/binary/but"
    if [ -d "$BUT_BIN.dSYM" ]; then
        cp -R "$BUT_BIN.dSYM" "$PERF_PROFILE_OUTPUT_DIR/binary/but.dSYM"
    fi
    BUT_BIN=$PERF_PROFILE_OUTPUT_DIR/binary/but
    {
        printf 'scenario: %s\nbackend: %s\nfixture: %s\n' "$scenario_name" "$backend" "$PERF_FIXTURE_COMMIT"
        printf 'harness: %s\n' "$("$GIT_BIN" -C "$REPO_ROOT" rev-parse HEAD)"
        printf 'os: %s\narchitecture: %s\n' "$(uname -sr)" "$(uname -m)"
        printf 'original binary: %s\nretained binary: %s\nbuild: %s\n' "$original_binary" "$BUT_BIN" "$build_description"
        printf 'binary checksum (cksum): '; cksum "$BUT_BIN"
        printf 'caller RUSTFLAGS: %s\ncaller CARGO_ENCODED_RUSTFLAGS: %s\n' "${RUSTFLAGS:-}" "${CARGO_ENCODED_RUSTFLAGS:-}"
        printf 'show output: %s\n' "${PERF_SHOW_OUTPUT:-0}"
        printf 'profiler: '; "$PERF_PROFILER_BIN" --version
        printf 'harness worktree changes:\n'; "$GIT_BIN" -C "$REPO_ROOT" status --short
    } >"$PERF_PROFILE_OUTPUT_DIR/metadata.txt"

    trap 'status=$?; printf "runner exit status: %s\n" "$status" >>"$PERF_PROFILE_OUTPUT_DIR/metadata.txt"; perf_cleanup_session' EXIT
    # Backend children may resolve perf/xctrace through PATH. Keep resolved absolute
    # tool directory first, including when caller used a relative PATH entry.
    PATH="$(dirname "$PERF_PROFILER_BIN"):$PATH"
    set --
    if [ -n "${DEVELOPER_DIR:-}" ]; then
        set -- "DEVELOPER_DIR=$DEVELOPER_DIR"
    fi
    status=0
    perf_run_isolated profile_wait "$@" \
        PERF_SHOW_OUTPUT="${PERF_SHOW_OUTPUT:-0}" \
        PERF_PROFILE_BACKEND="$backend" \
        PERF_PROFILER_BIN="$PERF_PROFILER_BIN" \
        PERF_PROFILE_OUTPUT_DIR="$PERF_PROFILE_OUTPUT_DIR" \
        "$PERF_ROOT/profile.sh" "$backend" "$scenario_name" || status=$?
    printf 'Artifacts retained: %s\n' "$PERF_PROFILE_OUTPUT_DIR" >&2
    [ "$status" -eq 0 ] || exit "$status"
    case "$backend" in
        samply) printf 'View: samply load "%s/profile.json"\n' "$PERF_PROFILE_OUTPUT_DIR" ;;
        perf) printf 'View: perf report -i "%s/perf.data"\n' "$PERF_PROFILE_OUTPUT_DIR" ;;
        flamegraph) printf 'Open in browser: %s/flamegraph.svg\n' "$PERF_PROFILE_OUTPUT_DIR" ;;
    esac
    exit 0
fi

mkdir -p "$HOME" "$E2E_TEST_APP_DATA_DIR"
# Launch a tiny native operation before expensive fixture setup. This checks actual
# recording permissions (including macOS injection), not merely tool installation.
printf 'Checking profiler access...\n' >&2
mkdir "$PERF_PROFILE_OUTPUT_DIR/preflight"
status=0
PERF_PROFILE_EXEC=1 PERF_PROFILE_PREFLIGHT=1 PERF_PROFILE_OUTPUT_DIR="$PERF_PROFILE_OUTPUT_DIR/preflight" \
    profile_wait "$PERF_ROOT/profile.sh" "$BUT_BIN" --version || status=$?
[ ! -f "$PERF_PROFILE_OUTPUT_DIR/preflight/profiler.log" ] || cat "$PERF_PROFILE_OUTPUT_DIR/preflight/profiler.log" >&2
[ "$status" -eq 0 ] || perf_die 'profiler preflight failed; check recording permissions/tool version (macOS: prefer samply); no security settings were changed'

printf 'Creating immutable GitButler history fixture...\n' >&2
profile_wait perf_create_gitbutler_fixture "$PERF_FIXTURE_REPO" "$PERF_SOURCE_REPO" "$PERF_FIXTURE_COMMIT"
PERF_RUN_ROOT=$PERF_SESSION_ROOT/runs/$scenario_name
export PERF_RUN_ROOT
printf 'Smoke-testing %s...\n' "$scenario_name" >&2
profile_wait "$setup_script"
profile_wait "$test_script"
printf 'Preparing fresh profiling state...\n' >&2
profile_wait "$setup_script"
printf 'Profiling %s with %s...\n' "$scenario_name" "$backend" >&2
status=0
PERF_PROFILE_EXEC=1 profile_wait "$test_script" || status=$?
[ ! -f "$PERF_PROFILE_OUTPUT_DIR/profiler.log" ] || cat "$PERF_PROFILE_OUTPUT_DIR/profiler.log" >&2
printf 'profiler exit status: %s\n' "$status" >>"$PERF_PROFILE_OUTPUT_DIR/metadata.txt"
[ "$status" -eq 0 ] || exit "$status"
case "$backend" in
    samply) artifact=profile.json ;;
    perf) artifact=perf.data ;;
    flamegraph) artifact=flamegraph.svg ;;
esac
[ -s "$PERF_PROFILE_OUTPUT_DIR/$artifact" ] || perf_die "profiler produced no $artifact; inspect profiler.log"
# This is recorder success, not an independent assertion about child exit status:
# tools differ in how they report failed/signalled targets (notably xctrace).

