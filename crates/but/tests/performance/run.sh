#!/bin/sh
set -eu

PERF_ROOT=$(CDPATH='' cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(CDPATH='' cd "$PERF_ROOT/../../../.." && pwd)
# shellcheck disable=SC1091
. "$PERF_ROOT/lib.sh"

if [ "${PERF_ENV_ISOLATED:-}" != 1 ]; then
    perf_require_command git

    GIT_BIN=$(command -v git)
    perf_require_command hyperfine
    HYPERFINE_BIN=$(command -v hyperfine)
    perf_assert_full_oid "$PERF_FIXTURE_COMMIT"

    if [ -n "${PERF_UPLOAD_DB:-}" ]; then
        perf_require_command psql
        perf_require_command jq
    fi

    PERF_SESSION_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/but-performance.XXXXXX")
    PERF_SESSION_ROOT=$(CDPATH='' cd "$PERF_SESSION_ROOT" && pwd)
    # Invoked by EXIT trap in outer runner.
    # shellcheck disable=SC2329
    cleanup() {
        chmod -R u+w "$PERF_SESSION_ROOT" 2>/dev/null || true
        rm -rf "$PERF_SESSION_ROOT"
    }
    trap cleanup EXIT
    trap 'exit 130' INT
    trap 'exit 143' HUP TERM

    BINARY_VERSION=
    if [ -n "${PERF_CHANNEL:-}" ]; then
        perf_require_command curl
        perf_require_command jq
        release_url="https://app.gitbutler.com/api/downloads?limit=1&channel=$PERF_CHANNEL"
        if [ -n "${PERF_VERSION:-}" ]; then
            release_url="$release_url&version=$PERF_VERSION"
        fi
        curl -fsSL "$release_url" -o "$PERF_SESSION_ROOT/downloads.json"
        jq -e '.[0]' "$PERF_SESSION_ROOT/downloads.json" >"$PERF_SESSION_ROOT/release.json"
        download_url=$(jq -er '
            .builds[] | select(.os == "linux" and .arch == "x86_64" and .file == "but") | .url
        ' "$PERF_SESSION_ROOT/release.json")
        PERF_BINARY_COMMIT=$(jq -er .sha "$PERF_SESSION_ROOT/release.json")
        BINARY_VERSION=$(jq -er .version "$PERF_SESSION_ROOT/release.json")
        jq -r '"Downloading \(.channel) \(.version) (\(.build_version)), commit \(.sha)..."' \
            "$PERF_SESSION_ROOT/release.json" >&2
        BUT_BIN=$PERF_SESSION_ROOT/but
        curl -fsSL "$download_url" -o "$BUT_BIN"
        chmod +x "$BUT_BIN"
    elif [ -n "${BUT_BIN:-}" ]; then
        if [ -n "${PERF_UPLOAD_DB:-}" ]; then
            : "${PERF_BINARY_COMMIT:?set PERF_BINARY_COMMIT to supplied binary revision for upload}"
        fi
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
        if [ -n "${PERF_UPLOAD_DB:-}" ]; then
            PERF_BINARY_COMMIT=$("$GIT_BIN" -C "$REPO_ROOT" rev-parse HEAD)
        fi
    fi

    if [ -n "${PERF_UPLOAD_DB:-}" ]; then
        HARNESS_COMMIT=$("$GIT_BIN" -C "$REPO_ROOT" rev-parse HEAD)
        perf_assert_full_oid "$PERF_BINARY_COMMIT"
        MACHINE=${PERF_MACHINE:-$(uname -n)}
        OS=$(uname -sr)
        CPU=${PERF_CPU:-$(uname -m)}
    fi
    PATH_VALUE=$PATH
    WARMUP_VALUE=${PERF_WARMUP:-3}
    MIN_RUNS_VALUE=${PERF_MIN_RUNS:-20}
    RUNS_VALUE=${PERF_RUNS:-}
    SHOW_OUTPUT_VALUE=${PERF_SHOW_OUTPUT:-0}
    RESULTS_DIR_VALUE=${PERF_RESULTS_DIR:-}
    if [ -n "${PERF_UPLOAD_DB:-}" ] && [ -z "$RESULTS_DIR_VALUE" ]; then
        RESULTS_DIR_VALUE=$REPO_ROOT/target/performance-results
    fi
    case "$SHOW_OUTPUT_VALUE" in
        0|1) ;;
        *) perf_die "PERF_SHOW_OUTPUT must be 0 or 1" ;;
    esac
    if [ -n "$RESULTS_DIR_VALUE" ]; then
        mkdir -p "$RESULTS_DIR_VALUE"
        RESULTS_DIR_VALUE=$(CDPATH='' cd "$RESULTS_DIR_VALUE" && pwd)
        if [ -n "${PERF_CHANNEL:-}" ]; then
            cp "$PERF_SESSION_ROOT/release.json" "$RESULTS_DIR_VALUE/release.json"
        fi
    fi

    set +e
    env -i \
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
        PERF_RESULTS_DIR="$RESULTS_DIR_VALUE" \
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
    benchmark_status=$?
    set -e
    [ "$benchmark_status" -eq 0 ] || exit "$benchmark_status"

    if [ -n "${PERF_UPLOAD_DB:-}" ]; then
        while IFS= read -r scenario_name; do
            result_file=$RESULTS_DIR_VALUE/$scenario_name.json
            printf 'Uploading %s...\n' "$scenario_name" >&2
            jq -e '.results | length == 1' "$result_file" >/dev/null ||
                perf_die "expected one Hyperfine result in $result_file"
            result_json=$(jq -c . "$result_file")
            if ! PGCONNECT_TIMEOUT="${PGCONNECT_TIMEOUT:-10}" \
                psql --dbname="$PERF_UPLOAD_DB" -X -w -v ON_ERROR_STOP=1 \
                -v scenario="$scenario_name" \
                -v commit_sha="$PERF_BINARY_COMMIT" \
                -v version="$BINARY_VERSION" \
                -v fixture_sha="$PERF_FIXTURE_COMMIT" \
                -v harness_sha="$HARNESS_COMMIT" \
                -v machine="$MACHINE" -v os="$OS" -v cpu="$CPU" \
                -v warmup_count="$WARMUP_VALUE" -v hyperfine_json="$result_json" \
                -f "$PERF_ROOT/upload.sql"; then
                perf_die "upload failed; benchmark JSON retained in $RESULTS_DIR_VALUE"
            fi
        done <"$PERF_SESSION_ROOT/completed-scenarios"
    fi
    exit 0
fi

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

perf_benchmark_scenario() {
    if [ -n "$PERF_RUNS" ]; then
        set -- --warmup "$PERF_WARMUP" --runs "$PERF_RUNS"
    else
        set -- --warmup "$PERF_WARMUP" --min-runs "$PERF_MIN_RUNS"
    fi
    if [ -n "$PERF_RESULTS_DIR" ]; then
        set -- "$@" --export-json "$PERF_RESULTS_DIR/$scenario_name.json"
    fi
    perf_hyperfine "$@" \
        --prepare "$setup_script" \
        --shell=none \
        "$test_script"
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
    perf_benchmark_scenario
    printf '%s\n' "$scenario_name" >>"$PERF_SESSION_ROOT/completed-scenarios"
done
