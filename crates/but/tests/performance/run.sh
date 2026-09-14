#!/bin/sh
set +x
set -eu
# Recording/build tools must not inherit credentials from the launcher.
unset PERF_UPLOAD_TOKEN

PERF_ROOT=$(CDPATH='' cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(CDPATH='' cd "$PERF_ROOT/../../../.." && pwd)
# shellcheck disable=SC1091
. "$PERF_ROOT/lib.sh"

if [ "${PERF_ENV_ISOLATED:-}" != 1 ]; then
    for tool in git hyperfine jq; do perf_require_command "$tool"; done
    GIT_BIN=$(command -v git)
    HYPERFINE_BIN=$(command -v hyperfine)
    perf_create_session

    if [ -n "${PERF_RESULTS_DIR:-}" ]; then
        mkdir -p "$PERF_RESULTS_DIR"
        [ -z "$(ls -A "$PERF_RESULTS_DIR")" ] || perf_die 'PERF_RESULTS_DIR must be empty for a new benchmark run'
    else
        mkdir -p "$REPO_ROOT/target/performance-results"
        PERF_RESULTS_DIR=$(mktemp -d "$REPO_ROOT/target/performance-results/run.XXXXXX")
    fi
    PERF_RESULTS_DIR=$(CDPATH='' cd "$PERF_RESULTS_DIR" && pwd)
    printf 'Results: %s\n' "$PERF_RESULTS_DIR" >&2

    BINARY_VERSION=
    BINARY_CHANNEL=
    if [ -n "${PERF_CHANNEL:-}" ]; then
        perf_resolve_release
        download_url=$(jq -er '
            .builds[] | select(.os == "linux" and .arch == "x86_64" and .file == "but") | .url
        ' "$PERF_RESULTS_DIR/release.json")
        jq -r '"Downloading \(.channel) \(.version) (\(.build_version)), commit \(.sha)..."' \
            "$PERF_RESULTS_DIR/release.json" >&2
        BUT_BIN=$PERF_SESSION_ROOT/but
        curl -fsSL "$download_url" -o "$BUT_BIN"
        chmod +x "$BUT_BIN"
    elif [ -n "${BUT_BIN:-}" ]; then
        : "${PERF_BINARY_COMMIT:?set PERF_BINARY_COMMIT to supplied binary revision}"
        perf_resolve_binary
    else
        perf_require_command cargo
        printf 'Building optimized but binary...\n' >&2
        (cd "$REPO_ROOT" && cargo build --profile bench -p but)
        BUT_BIN=$REPO_ROOT/target/release/but
        perf_resolve_binary
        PERF_BINARY_COMMIT=$("$GIT_BIN" -C "$REPO_ROOT" rev-parse HEAD)
    fi
    # No filename in checksum: supplied local binary can live at a different path.
    cksum <"$BUT_BIN" >"$PERF_RESULTS_DIR/binary-checksum.txt"
    HARNESS_COMMIT=$("$GIT_BIN" -C "$REPO_ROOT" rev-parse HEAD)
    HARNESS_DIRTY=false
    if [ -n "$("$GIT_BIN" -C "$REPO_ROOT" status --porcelain --untracked-files=normal)" ]; then
        HARNESS_DIRTY=true
    fi
    perf_assert_full_oid "$PERF_BINARY_COMMIT"
    perf_assert_full_oid "$HARNESS_COMMIT"
    WARMUP_VALUE=${PERF_WARMUP:-3}
    MIN_RUNS_VALUE=${PERF_MIN_RUNS:-20}
    RUNS_VALUE=${PERF_RUNS:-}
    SHOW_OUTPUT_VALUE=${PERF_SHOW_OUTPUT:-0}
    SKIP_SMOKE_VALUE=${PERF_SKIP_SMOKE:-0}
    case "$SHOW_OUTPUT_VALUE" in
        0|1) ;;
        *) perf_die 'PERF_SHOW_OUTPUT must be 0 or 1' ;;
    esac
    case "$SKIP_SMOKE_VALUE" in
        0|1) ;;
        *) perf_die 'PERF_SKIP_SMOKE must be 0 or 1' ;;
    esac
    # Execution identity belongs to saved results, not to an upload attempt.
    run_id=$(od -An -N16 -tx1 /dev/urandom | tr -d ' \n')
    [ "${#run_id}" -eq 32 ] || perf_die 'could not generate run ID'
    jq -n \
        --arg uploadId "$run_id" --arg timestamp "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" \
        --arg binarySha "$PERF_BINARY_COMMIT" --arg version "$BINARY_VERSION" \
        --arg channel "$BINARY_CHANNEL" --arg harnessSha "$HARNESS_COMMIT" \
        --arg fixtureSha "$PERF_FIXTURE_COMMIT" --argjson dirty "$HARNESS_DIRTY" \
        --arg machine "${PERF_MACHINE:-$(uname -n)}" --arg os "$(uname -sr)" \
        --arg cpu "${PERF_CPU:-$(uname -m)}" --arg arch "$(uname -m)" \
        --arg warmup "$WARMUP_VALUE" --arg hyperfineVersion "$("$HYPERFINE_BIN" --version)" \
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
    ' >"$PERF_RESULTS_DIR/metadata.json"

    perf_run_isolated command \
        PERF_WARMUP="$WARMUP_VALUE" \
        PERF_MIN_RUNS="$MIN_RUNS_VALUE" \
        PERF_RUNS="$RUNS_VALUE" \
        PERF_SHOW_OUTPUT="$SHOW_OUTPUT_VALUE" \
        PERF_SKIP_SMOKE="$SKIP_SMOKE_VALUE" \
        PERF_RESULTS_DIR="$PERF_RESULTS_DIR" \
        HYPERFINE_BIN="$HYPERFINE_BIN" \
        "$0" "$@"
    printf 'Benchmarks saved: %s\n' "$PERF_RESULTS_DIR"
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

scenario_root=$PERF_ROOT/scenarios
if [ "$#" -eq 0 ]; then
    set --
    for scenario_dir in "$scenario_root"/*; do
        [ -d "$scenario_dir" ] || continue
        set -- "$@" "$(basename "$scenario_dir")"
    done
fi
[ "$#" -gt 0 ] || perf_die 'no performance scenarios found'
# Create directories up front: incomplete runs cannot be mistaken for complete ones.
for scenario_name; do
    perf_validate_scenario "$scenario_name"
    mkdir "$PERF_RESULTS_DIR/$scenario_name"
done

for scenario_name; do
    setup_script=$scenario_root/$scenario_name/setup.sh
    test_script=$scenario_root/$scenario_name/test.sh
    PERF_RUN_ROOT=$PERF_SESSION_ROOT/runs/$scenario_name
    export PERF_RUN_ROOT

    if [ "$PERF_SKIP_SMOKE" = 0 ]; then
        printf '\nSmoke-testing %s...\n' "$scenario_name" >&2
        "$setup_script"
        "$test_script"
    fi
    printf 'Benchmarking %s...\n' "$scenario_name" >&2
    # Subshell preserves selected scenario arguments for the outer loop.
    (
        if [ -n "$PERF_RUNS" ]; then
            set -- --warmup "$PERF_WARMUP" --runs "$PERF_RUNS"
        else
            set -- --warmup "$PERF_WARMUP" --min-runs "$PERF_MIN_RUNS"
        fi
        perf_hyperfine "$@" \
            --export-json "$PERF_RESULTS_DIR/$scenario_name/benchmark.json.tmp" \
            --prepare "$setup_script" --shell=none "$test_script"
        mv "$PERF_RESULTS_DIR/$scenario_name/benchmark.json.tmp" "$PERF_RESULTS_DIR/$scenario_name/benchmark.json"
    )
done
