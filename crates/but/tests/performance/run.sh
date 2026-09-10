#!/bin/sh
# Upload credentials must never appear in shell tracing.
set +x
set -eu

PERF_ROOT=$(CDPATH='' cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(CDPATH='' cd "$PERF_ROOT/../../../.." && pwd)
# shellcheck disable=SC1091
. "$PERF_ROOT/lib.sh"

if [ "${PERF_ENV_ISOLATED:-}" != 1 ]; then
    UPLOAD_ENABLED=0
    if [ -n "${PERF_UPLOAD_URL:-}" ] || [ -n "${PERF_UPLOAD_TOKEN:-}" ]; then
        "$PERF_ROOT/upload.sh" --check
        UPLOAD_ENABLED=1
        SESSION_TIMESTAMP=$(date -u '+%Y-%m-%dT%H:%M:%SZ')
        # Random execution identity, independent of binary revision and date.
        SESSION_ID=$(od -An -N16 -tx1 /dev/urandom | tr -d ' \n')
        [ "${#SESSION_ID}" -eq 32 ] || perf_die 'could not generate upload ID'
    fi
    perf_require_command git

    GIT_BIN=$(command -v git)
    perf_require_command hyperfine
    HYPERFINE_BIN=$(command -v hyperfine)
    perf_assert_full_oid "$PERF_FIXTURE_COMMIT"

    perf_create_session

    BINARY_VERSION=
    BINARY_CHANNEL=
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
        BINARY_CHANNEL=$(jq -er .channel "$PERF_SESSION_ROOT/release.json")
        jq -r '"Downloading \(.channel) \(.version) (\(.build_version)), commit \(.sha)..."' \
            "$PERF_SESSION_ROOT/release.json" >&2
        BUT_BIN=$PERF_SESSION_ROOT/but
        curl -fsSL "$download_url" -o "$BUT_BIN"
        chmod +x "$BUT_BIN"
    elif [ -n "${BUT_BIN:-}" ]; then
        if [ "$UPLOAD_ENABLED" = 1 ]; then
            : "${PERF_BINARY_COMMIT:?set PERF_BINARY_COMMIT to supplied binary revision for upload}"
        fi
        perf_resolve_binary
    else
        perf_require_command cargo
        printf 'Building optimized but binary...\n' >&2
        (cd "$REPO_ROOT" && cargo build --profile bench -p but)
        BUT_BIN=$REPO_ROOT/target/release/but
        [ -x "$BUT_BIN" ] || perf_die "bench-profile binary not found: $BUT_BIN"
        if [ "$UPLOAD_ENABLED" = 1 ]; then
            PERF_BINARY_COMMIT=$("$GIT_BIN" -C "$REPO_ROOT" rev-parse HEAD)
        fi
    fi

    if [ "$UPLOAD_ENABLED" = 1 ]; then
        HARNESS_COMMIT=$("$GIT_BIN" -C "$REPO_ROOT" rev-parse HEAD)
        HARNESS_DIRTY=false
        if [ -n "$("$GIT_BIN" -C "$REPO_ROOT" status --porcelain --untracked-files=normal)" ]; then
            HARNESS_DIRTY=true
        fi
        perf_assert_full_oid "$PERF_BINARY_COMMIT"
        perf_assert_full_oid "$HARNESS_COMMIT"
        MACHINE=${PERF_MACHINE:-$(uname -n)}
        OS=$(uname -sr)
        ARCH=$(uname -m)
        CPU=${PERF_CPU:-$ARCH}
        HYPERFINE_VERSION=$("$HYPERFINE_BIN" --version)
    fi
    WARMUP_VALUE=${PERF_WARMUP:-3}
    MIN_RUNS_VALUE=${PERF_MIN_RUNS:-20}
    RUNS_VALUE=${PERF_RUNS:-}
    SHOW_OUTPUT_VALUE=${PERF_SHOW_OUTPUT:-0}
    RESULTS_DIR_VALUE=${PERF_RESULTS_DIR:-}
    if [ "$UPLOAD_ENABLED" = 1 ] && [ -z "$RESULTS_DIR_VALUE" ]; then
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

    BENCHMARK_RESULTS_DIR=$RESULTS_DIR_VALUE
    if [ "$UPLOAD_ENABLED" = 1 ]; then
        UPLOAD_DIR=$RESULTS_DIR_VALUE/uploads/$SESSION_ID
        # mkdir without -p makes an unlikely ID collision fail rather than overwrite.
        mkdir -p "$RESULTS_DIR_VALUE/uploads"
        mkdir "$UPLOAD_DIR"
        BENCHMARK_RESULTS_DIR=$UPLOAD_DIR/scenarios
        mkdir "$BENCHMARK_RESULTS_DIR"
        jq -n \
            --arg uploadId "$SESSION_ID" --arg timestamp "$SESSION_TIMESTAMP" \
            --arg binarySha "$PERF_BINARY_COMMIT" --arg version "$BINARY_VERSION" \
            --arg channel "$BINARY_CHANNEL" --arg harnessSha "$HARNESS_COMMIT" \
            --arg fixtureSha "$PERF_FIXTURE_COMMIT" --argjson dirty "$HARNESS_DIRTY" \
            --arg machine "$MACHINE" --arg os "$OS" --arg cpu "$CPU" --arg arch "$ARCH" \
            --arg warmup "$WARMUP_VALUE" --arg hyperfineVersion "$HYPERFINE_VERSION" \
            --argjson showOutput "$SHOW_OUTPUT_VALUE" '
            ($warmup | tonumber) as $warmupCount |
            if $warmupCount < 0 or ($warmupCount | floor) != $warmupCount then
                error("PERF_WARMUP must be a nonnegative integer")
            else {
                schemaVersion: 1, uploadId: $uploadId, timestamp: $timestamp,
                timestampKind: "measured",
                binary: ({commitSha: $binarySha} +
                    (if $version == "" then {} else {version: $version} end) +
                    (if $channel == "" then {} else {channel: $channel} end)),
                harness: {commitSha: $harnessSha, fixtureSha: $fixtureSha, dirty: $dirty},
                machine: {name: $machine, os: $os, cpu: $cpu, arch: $arch},
                config: {warmupCount: $warmupCount, hyperfineVersion: $hyperfineVersion,
                    showOutput: ($showOutput == 1)}
            } end
        ' >"$UPLOAD_DIR/metadata.json"
    fi

    set +e
    perf_run_isolated command \
        PERF_WARMUP="$WARMUP_VALUE" \
        PERF_MIN_RUNS="$MIN_RUNS_VALUE" \
        PERF_RUNS="$RUNS_VALUE" \
        PERF_SHOW_OUTPUT="$SHOW_OUTPUT_VALUE" \
        PERF_RESULTS_DIR="$BENCHMARK_RESULTS_DIR" \
        HYPERFINE_BIN="$HYPERFINE_BIN" \
        "$0" "$@"
    benchmark_status=$?
    set -e
    [ "$benchmark_status" -eq 0 ] || exit "$benchmark_status"

    if [ "$UPLOAD_ENABLED" = 1 ]; then
        cp "$PERF_SESSION_ROOT/completed-scenarios" "$UPLOAD_DIR/completed-scenarios"
        while IFS= read -r scenario_name; do
            result_file=$BENCHMARK_RESULTS_DIR/$scenario_name.json
            # Keep the familiar latest-result exports; payload uses execution-local files.
            cp "$result_file" "$RESULTS_DIR_VALUE/$scenario_name.json"
            jq -cn --arg scenario "$scenario_name" --slurpfile hyperfine "$result_file" '
                if ($hyperfine | length) != 1 or ($hyperfine[0].results | length) != 1 then
                    error("expected one Hyperfine document with one result")
                else {scenario: $scenario, hyperfine: $hyperfine[0]} end
            ' >>"$UPLOAD_DIR/measurements.jsonl"
        done <"$UPLOAD_DIR/completed-scenarios"
        jq --slurpfile measurements "$UPLOAD_DIR/measurements.jsonl" '
            . + {measurements: $measurements}
        ' "$UPLOAD_DIR/metadata.json" >"$UPLOAD_DIR/payload.json.tmp"
        mv "$UPLOAD_DIR/payload.json.tmp" "$UPLOAD_DIR/payload.json"
        "$PERF_ROOT/upload.sh" "$UPLOAD_DIR/payload.json"
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
    perf_validate_scenario "$scenario_name"
    setup_script=$scenario_root/$scenario_name/setup.sh
    test_script=$scenario_root/$scenario_name/test.sh

    PERF_RUN_ROOT=$PERF_SESSION_ROOT/runs/$scenario_name
    export PERF_RUN_ROOT

    printf '\nSmoke-testing %s...\n' "$scenario_name" >&2
    "$setup_script"
    "$test_script"

    printf 'Benchmarking %s...\n' "$scenario_name" >&2
    perf_benchmark_scenario
    printf '%s\n' "$scenario_name" >>"$PERF_SESSION_ROOT/completed-scenarios"
done
