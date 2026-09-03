#!/bin/sh
set -eu

PERF_ROOT=$(CDPATH='' cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(CDPATH='' cd "$PERF_ROOT/../../../.." && pwd)
# shellcheck disable=SC1091
. "$PERF_ROOT/lib.sh"

if [ "${PERF_ENV_ISOLATED:-}" != 1 ]; then
    perf_require_command git
    perf_require_command hyperfine

    GIT_BIN=$(command -v git)
    HYPERFINE_BIN=$(command -v hyperfine)
    perf_assert_full_oid "$PERF_FIXTURE_COMMIT"

    if [ -n "${BUT_BIN:-}" ]; then
        case "$BUT_BIN" in
            /*) ;;
            *) BUT_BIN=$(CDPATH='' cd "$(dirname "$BUT_BIN")" && pwd)/$(basename "$BUT_BIN") ;;
        esac
        [ -x "$BUT_BIN" ] || perf_die "BUT_BIN is not executable: $BUT_BIN"
    else
        perf_require_command cargo
        printf 'Building optimized but binary...\n' >&2
        (cd "$REPO_ROOT" && cargo build --profile bench -p but)
        BUT_BIN=$REPO_ROOT/target/release/but
        [ -x "$BUT_BIN" ] || perf_die "bench-profile binary not found: $BUT_BIN"
    fi

    PERF_SESSION_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/but-performance.XXXXXX")
    PATH_VALUE=$PATH
    WARMUP_VALUE=${PERF_WARMUP:-3}
    MIN_RUNS_VALUE=${PERF_MIN_RUNS:-20}
    RUNS_VALUE=${PERF_RUNS:-}
    SHOW_OUTPUT_VALUE=${PERF_SHOW_OUTPUT:-0}
    case "$SHOW_OUTPUT_VALUE" in
        0|1) ;;
        *) perf_die "PERF_SHOW_OUTPUT must be 0 or 1" ;;
    esac

    exec env -i \
        PATH="$PATH_VALUE" \
        PERF_ENV_ISOLATED=1 \
        PERF_ROOT="$PERF_ROOT" \
        PERF_SOURCE_REPO="$REPO_ROOT" \
        PERF_SESSION_ROOT="$PERF_SESSION_ROOT" \
        PERF_FIXTURE_REPO="$PERF_SESSION_ROOT/fixture.git" \
        PERF_FIXTURE_COMMIT="$PERF_FIXTURE_COMMIT" \
        PERF_WARMUP="$WARMUP_VALUE" \
        PERF_MIN_RUNS="$MIN_RUNS_VALUE" \
        PERF_RUNS="$RUNS_VALUE" \
        PERF_SHOW_OUTPUT="$SHOW_OUTPUT_VALUE" \
        BUT_BIN="$BUT_BIN" \
        GIT_BIN="$GIT_BIN" \
        HYPERFINE_BIN="$HYPERFINE_BIN" \
        HOME="$PERF_SESSION_ROOT/harness-home" \
        E2E_TEST_APP_DATA_DIR="$PERF_SESSION_ROOT/harness-app-data" \
        GIT_CONFIG_NOSYSTEM=1 \
        GIT_CONFIG_GLOBAL=/dev/null \
        GIT_ATTR_NOSYSTEM=1 \
        GIT_TERMINAL_PROMPT=0 \
        GIT_CONFIG_COUNT=4 \
        GIT_CONFIG_KEY_0=commit.gpgsign \
        GIT_CONFIG_VALUE_0=false \
        GIT_CONFIG_KEY_1=tag.gpgsign \
        GIT_CONFIG_VALUE_1=false \
        GIT_CONFIG_KEY_2=init.defaultBranch \
        GIT_CONFIG_VALUE_2=main \
        GIT_CONFIG_KEY_3=protocol.file.allow \
        GIT_CONFIG_VALUE_3=always \
        TZ=UTC \
        LANG=C \
        LC_ALL=C \
        NO_BG_TASKS=1 \
        NOPAGER=1 \
        "$0" "$@"
fi

cleanup() {
    chmod -R u+w "$PERF_SESSION_ROOT" 2>/dev/null || true
    rm -rf "$PERF_SESSION_ROOT"
}
trap cleanup EXIT HUP INT TERM

mkdir -p "$HOME" "$E2E_TEST_APP_DATA_DIR"
printf 'Fixture commit: %s\n' "$PERF_FIXTURE_COMMIT"
printf 'Benchmark binary: %s\n' "$BUT_BIN"
printf 'Creating immutable GitButler history fixture...\n' >&2
perf_create_gitbutler_fixture "$PERF_FIXTURE_REPO" "$PERF_SOURCE_REPO" "$PERF_FIXTURE_COMMIT"

perf_hyperfine() {
    if [ "$PERF_SHOW_OUTPUT" = 1 ]; then
        "$HYPERFINE_BIN" --show-output "$@"
    else
        "$HYPERFINE_BIN" "$@"
    fi
}

scenario_root=$PERF_ROOT/scenarios
if [ "$#" -eq 0 ]; then
    set --
    for scenario_dir in "$scenario_root"/*; do
        [ -d "$scenario_dir" ] || continue
        set -- "$@" "$(basename "$scenario_dir")"
    done
fi
[ "$#" -gt 0 ] || perf_die "no performance scenarios found"

for scenario_name in "$@"; do
    case "$scenario_name" in
        ''|*/*|.*) perf_die "invalid scenario name: $scenario_name" ;;
    esac

    scenario_dir=$scenario_root/$scenario_name
    setup_script=$scenario_dir/setup.sh
    test_script=$scenario_dir/test.sh
    [ -x "$setup_script" ] || perf_die "scenario setup is not executable: $setup_script"
    [ -x "$test_script" ] || perf_die "scenario test is not executable: $test_script"

    PERF_RUN_ROOT=$PERF_SESSION_ROOT/runs/$scenario_name
    export PERF_RUN_ROOT

    printf '\nSmoke-testing %s...\n' "$scenario_name" >&2
    "$setup_script"
    "$test_script"

    printf 'Benchmarking %s...\n' "$scenario_name" >&2
    if [ -n "$PERF_RUNS" ]; then
        perf_hyperfine \
            --warmup "$PERF_WARMUP" \
            --runs "$PERF_RUNS" \
            --prepare "$setup_script" \
            --shell=none \
            "$test_script"
    else
        perf_hyperfine \
            --warmup "$PERF_WARMUP" \
            --min-runs "$PERF_MIN_RUNS" \
            --prepare "$setup_script" \
            --shell=none \
            "$test_script"
    fi
done
