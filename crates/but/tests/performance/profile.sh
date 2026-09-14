#!/bin/sh
set +x
set -eu
# Recording/build tools must not inherit credentials from the launcher.
unset PERF_UPLOAD_TOKEN

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
        samply-presymbolicate)
            set -- "$PERF_PROFILER_BIN" record --save-only --presymbolicate -o "$PWD/profile.json" -- "$@"
            ;;
        perf)
            set -- "$PERF_PROFILER_BIN" record -o "$PWD/perf.data" --call-graph dwarf -- "$@"
            ;;
        flamegraph)
            if [ "${PERF_PROFILE_PREFLIGHT:-}" = 1 ]; then
                # A --version process may yield zero samples: check recording access,
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
    # macOS Samply must inject directly into locally built but, not a shell wrapper.
    if [ "$PERF_SHOW_OUTPUT" = 1 ]; then
        exec "$@" 2>"$PERF_PROFILE_OUTPUT_DIR/profiler.log"
    else
        exec "$@" >/dev/null 2>"$PERF_PROFILE_OUTPUT_DIR/profiler.log"
    fi
fi

# Stop descendants first, while parents can still reap them.
profile_stop() (
    for descendant in $(ps -e -o pid= -o ppid= | awk -v parent="$1" '$2 == parent { print $1 }'); do
        profile_stop "$descendant"
    done
    kill -TERM "$1" 2>/dev/null || true
)

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

profile_bounded() {
    if [ -n "${PERF_PROFILE_TIMEOUT:-}" ]; then
        timeout --kill-after=10 "$PERF_PROFILE_TIMEOUT" "$@"
    else
        "$@"
    fi
}

[ "$#" -ge 1 ] || perf_die 'usage: profile.sh <samply|samply-presymbolicate|perf|flamegraph> [scenario ...]'
backend=$1
shift
case "$backend" in
    samply|samply-presymbolicate|perf|flamegraph) ;;
    *) perf_die "unknown profiling backend: $backend" ;;
esac

if [ "${PERF_ENV_ISOLATED:-}" != 1 ]; then
    case "$(uname -s):$backend" in
        Linux:*|Darwin:samply|Darwin:samply-presymbolicate|Darwin:flamegraph) ;;
        *) perf_die "$backend is not supported on $(uname -s)" ;;
    esac
    case "${PERF_SHOW_OUTPUT:-0}" in
        0|1) ;;
        *) perf_die 'PERF_SHOW_OUTPUT must be 0 or 1' ;;
    esac
    for tool in git jq; do perf_require_command "$tool"; done
    GIT_BIN=$(command -v git)
    GIT_BIN=$(CDPATH='' cd "$(dirname "$GIT_BIN")" && pwd)/$(basename "$GIT_BIN")
    profiler=$backend
    [ "$backend" != samply-presymbolicate ] || profiler=samply
    perf_require_command "$profiler"
    PERF_PROFILER_BIN=$(command -v "$profiler")
    PERF_PROFILER_BIN=$(CDPATH='' cd "$(dirname "$PERF_PROFILER_BIN")" && pwd)/$(basename "$PERF_PROFILER_BIN")
    unset GIT_DIR GIT_INDEX_FILE GIT_OBJECT_DIRECTORY
    unset GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_WORK_TREE GIT_COMMON_DIR
    "$GIT_BIN" -C "$REPO_ROOT" cat-file -e "$PERF_FIXTURE_COMMIT^{commit}" ||
        perf_die "fixture commit missing from source checkout: $PERF_FIXTURE_COMMIT"

    if [ -n "${PERF_RESULTS_DIR:-}" ]; then
        mkdir -p "$PERF_RESULTS_DIR"
    else
        mkdir -p "$REPO_ROOT/target/performance-results"
        PERF_RESULTS_DIR=$(mktemp -d "$REPO_ROOT/target/performance-results/profile.XXXXXX")
    fi
    PERF_RESULTS_DIR=$(CDPATH='' cd "$PERF_RESULTS_DIR" && pwd)
    paired=0
    if [ -f "$PERF_RESULTS_DIR/metadata.json" ]; then
        paired=1
        jq -e --arg fixture "$PERF_FIXTURE_COMMIT" '
            .schemaVersion == 1 and (.uploadId | type == "string" and length > 0) and
            .harness.fixtureSha == $fixture
        ' "$PERF_RESULTS_DIR/metadata.json" >/dev/null || perf_die 'invalid run metadata or mismatched fixture'
    fi
    if [ "$#" -eq 0 ]; then
        if [ "$paired" = 1 ]; then
            scenario_root=$PERF_RESULTS_DIR
        else
            scenario_root=$PERF_ROOT/scenarios
        fi
        for scenario_dir in "$scenario_root"/*; do
            [ -d "$scenario_dir" ] || continue
            set -- "$@" "$(basename "$scenario_dir")"
        done
    fi
    [ "$#" -gt 0 ] || perf_die 'no scenarios found'
    selected=
    for scenario_name; do
        perf_validate_scenario "$scenario_name"
        case " $selected " in
            *" $scenario_name "*) perf_die "duplicate scenario: $scenario_name" ;;
        esac
        selected="$selected $scenario_name"
        if [ "$paired" = 1 ]; then
            [ -s "$PERF_RESULTS_DIR/$scenario_name/benchmark.json" ] ||
                perf_die "missing benchmark for $scenario_name"
        fi
        [ ! -e "$PERF_RESULTS_DIR/$scenario_name/profile-status" ] ||
            perf_die "profile already attempted for $scenario_name; use a new results directory"
        mkdir -p "$PERF_RESULTS_DIR/$scenario_name"
    done
    perf_create_session
    # Validate release selection before reserving persistent profiling output, so
    # corrected settings can retry against the same benchmark results.
    build_version=
    if [ -n "${PERF_CHANNEL:-}" ] || [ -f "$PERF_RESULTS_DIR/release.json" ]; then
        [ "$(uname -s):$(uname -m)" = Linux:x86_64 ] || perf_die 'release profiling requires Linux x86_64'
        for tool in curl tar; do perf_require_command "$tool"; done
        perf_resolve_release
        if [ "$paired" = 1 ]; then
            jq -e --arg channel "$BINARY_CHANNEL" --arg version "$BINARY_VERSION" --arg sha "$PERF_BINARY_COMMIT" '
                .binary.channel == $channel and .binary.version == $version and .binary.commitSha == $sha
            ' "$PERF_RESULTS_DIR/metadata.json" >/dev/null || perf_die 'release does not match benchmark'
        fi
        build_version=$(jq -er '
            select(.channel == "nightly") |
            .build_version | select(test("^[0-9]+[.][0-9]+[.][0-9]+-[0-9]+$"))
        ' "$PERF_RESULTS_DIR/release.json") || perf_die 'profiling artifacts require a nightly release'
    fi
    # Hidden support directory is not a scenario. Retain binary/symbols for viewers.
    mkdir "$PERF_RESULTS_DIR/.profiling" || perf_die 'profiling artifacts already exist'
    profile_root=$PERF_RESULTS_DIR/.profiling

    if [ "$backend" = samply-presymbolicate ]; then
        "$PERF_PROFILER_BIN" record --help >"$profile_root/samply-help.txt"
        grep -Eq -- '(^|[[:space:]])--presymbolicate([[:space:]]|$)' "$profile_root/samply-help.txt" ||
            perf_die 'Samply needs self-contained --presymbolicate; pin a supported build (see README)'
    fi
    if [ "$backend" = flamegraph ]; then
        case "$(uname -s)" in
            Linux) perf_require_command perf ;;
            Darwin)
                perf_require_command xcrun
                xcrun xctrace version || perf_die 'install/select full Xcode with Instruments, or use samply'
                xcrun xctrace list templates | grep -q 'Time Profiler' || perf_die 'Time Profiler template unavailable'
                ;;
        esac
    fi

    build_description='supplied binary'
    if [ -n "$build_version" ]; then
        PERF_PROFILE_TIMEOUT=${PERF_PROFILE_TIMEOUT:-300}
        printf 'Downloading profiling binary for %s...\n' "$build_version" >&2
        profile_wait curl --disable --fail --silent --show-error --connect-timeout 10 --max-time 300 \
            --retry 3 --retry-max-time 600 \
            "https://releases.gitbutler.com/profiling/nightly/$build_version/but-profiling.tar.gz" \
            -o "$PERF_SESSION_ROOT/profiling.tar.gz"
        tar -xzf "$PERF_SESSION_ROOT/profiling.tar.gz" -C "$profile_root" but
        BUT_BIN=$profile_root/but
        chmod +x "$BUT_BIN"
        build_description="nightly $build_version"
    else
        if [ -n "${BUT_BIN:-}" ]; then
            perf_resolve_binary
        elif [ "$paired" = 1 ]; then
            perf_die 'set BUT_BIN to the binary used for this benchmark run'
        else
            for tool in cargo rustc; do perf_require_command "$tool"; done
            host=$(rustc -vV | awk '/^host:/ { print $2 }')
            [ -n "$host" ] || perf_die 'could not determine native Rust target'
            build_dir=$REPO_ROOT/target/profiling-build
            build_description="bench profile, debug=true, strip=none, native target=$host"
            (
                cd "$REPO_ROOT"
                export CARGO_PROFILE_BENCH_DEBUG=true CARGO_PROFILE_BENCH_STRIP=none
                if [ "$(uname -s)" = Darwin ]; then export CARGO_PROFILE_BENCH_SPLIT_DEBUGINFO=packed; fi
                cargo build --profile bench -p but --bin but --target "$host" --target-dir "$build_dir"
            )
            BUT_BIN=$build_dir/$host/release/but
            perf_resolve_binary
        fi
        if [ "$paired" = 1 ]; then
            [ "$(cksum <"$BUT_BIN")" = "$(cat "$PERF_RESULTS_DIR/binary-checksum.txt")" ] ||
                perf_die 'supplied binary does not match benchmark binary'
        fi
        cp "$BUT_BIN" "$profile_root/but"
        if [ -d "$BUT_BIN.dSYM" ]; then cp -R "$BUT_BIN.dSYM" "$profile_root/but.dSYM"; fi
        BUT_BIN=$profile_root/but
    fi
    if [ -n "${PERF_PROFILE_TIMEOUT:-}" ]; then
        case "$PERF_PROFILE_TIMEOUT" in
            *[!0-9]*|0) perf_die 'PERF_PROFILE_TIMEOUT must be positive integer seconds' ;;
        esac
        [ "$PERF_PROFILE_TIMEOUT" -gt 0 ] || perf_die 'PERF_PROFILE_TIMEOUT must be positive'
        perf_require_command timeout
    fi
    {
        printf 'backend: %s\nfixture: %s\nbuild: %s\n' "$backend" "$PERF_FIXTURE_COMMIT" "$build_description"
        printf 'profiler: '; "$PERF_PROFILER_BIN" --version
        printf 'binary checksum: '; cksum "$BUT_BIN"
    } >"$profile_root/metadata.txt"
    PATH="$(dirname "$PERF_PROFILER_BIN"):$PATH"
    status=0
    perf_run_isolated profile_wait \
        DEVELOPER_DIR="${DEVELOPER_DIR:-}" \
        PERF_SHOW_OUTPUT="${PERF_SHOW_OUTPUT:-0}" \
        PERF_PROFILE_BACKEND="$backend" \
        PERF_PROFILER_BIN="$PERF_PROFILER_BIN" \
        PERF_PROFILE_TIMEOUT="${PERF_PROFILE_TIMEOUT:-}" \
        PERF_RESULTS_DIR="$PERF_RESULTS_DIR" \
        "$PERF_ROOT/profile.sh" "$backend" "$@" || status=$?
    printf 'Profiles saved: %s\n' "$PERF_RESULTS_DIR" >&2
    exit "$status"
fi

mkdir -p "$HOME" "$E2E_TEST_APP_DATA_DIR"
printf 'Checking profiler access...\n' >&2
mkdir "$PERF_RESULTS_DIR/.profiling/preflight"
PERF_PROFILE_EXEC=1 PERF_PROFILE_PREFLIGHT=1 PERF_PROFILE_OUTPUT_DIR="$PERF_RESULTS_DIR/.profiling/preflight" \
    profile_wait profile_bounded "$PERF_ROOT/profile.sh" "$BUT_BIN" --version ||
    perf_die "profiler preflight failed; inspect $PERF_RESULTS_DIR/.profiling/preflight/profiler.log"
printf 'Creating immutable GitButler history fixture...\n' >&2
profile_wait perf_create_gitbutler_fixture "$PERF_FIXTURE_REPO" "$PERF_SOURCE_REPO" "$PERF_FIXTURE_COMMIT"

failed=0
captured=0
for scenario_name; do
    PERF_RUN_ROOT=$PERF_SESSION_ROOT/runs/$scenario_name
    PERF_PROFILE_OUTPUT_DIR=$PERF_RESULTS_DIR/$scenario_name
    export PERF_RUN_ROOT PERF_PROFILE_OUTPUT_DIR
    # A killed/failed recorder must not leave an uploadable partial capture.
    printf '1\n' >"$PERF_PROFILE_OUTPUT_DIR/profile-status"
    printf 'Profiling %s with %s...\n' "$scenario_name" "$backend" >&2
    status=0
    profile_wait profile_bounded "$PERF_ROOT/scenarios/$scenario_name/setup.sh" \
        >"$PERF_PROFILE_OUTPUT_DIR/setup.log" 2>&1 || status=$?
    if [ "$status" -eq 0 ]; then
        PERF_PROFILE_EXEC=1 profile_wait profile_bounded "$PERF_ROOT/scenarios/$scenario_name/test.sh" || status=$?
    fi
    if [ "$status" -eq 0 ]; then
        case "$backend" in
            samply|samply-presymbolicate)
                artifact=profile.json
                if [ ! -f "$PERF_PROFILE_OUTPUT_DIR/profile.json" ] && [ -f "$PERF_PROFILE_OUTPUT_DIR/profile.json.gz" ]; then
                    if gzip -dc "$PERF_PROFILE_OUTPUT_DIR/profile.json.gz" >"$PERF_PROFILE_OUTPUT_DIR/profile.json.tmp"; then
                        mv "$PERF_PROFILE_OUTPUT_DIR/profile.json.tmp" "$PERF_PROFILE_OUTPUT_DIR/profile.json"
                    else
                        status=1
                    fi
                fi
                if [ "$backend" = samply-presymbolicate ]; then
                    perf_validate_profile "$PERF_PROFILE_OUTPUT_DIR/profile.json" || status=$?
                fi
                ;;
            perf) artifact=perf.data ;;
            flamegraph) artifact=flamegraph.svg ;;
        esac
        [ -s "$PERF_PROFILE_OUTPUT_DIR/$artifact" ] || status=1
    fi
    if [ "$status" -eq 0 ] && [ -f "$PERF_RESULTS_DIR/metadata.json" ]; then
        jq --arg scenario "$scenario_name" --arg backend "$backend" \
            '{uploadId, binarySha: .binary.commitSha, scenario: $scenario, backend: $backend}' \
            "$PERF_RESULTS_DIR/metadata.json" >"$PERF_PROFILE_OUTPUT_DIR/profile-metadata.json" || status=$?
    fi
    printf '%s\n' "$status" >"$PERF_PROFILE_OUTPUT_DIR/profile-status"
    if [ "$status" -eq 0 ]; then
        captured=$((captured + 1))
        case "$backend" in
            samply|samply-presymbolicate) printf 'View: samply load "%s/profile.json"\n' "$PERF_PROFILE_OUTPUT_DIR" ;;
            perf) printf 'View: perf report -i "%s/perf.data"\n' "$PERF_PROFILE_OUTPUT_DIR" ;;
            flamegraph) printf 'Open in browser: %s/flamegraph.svg\n' "$PERF_PROFILE_OUTPUT_DIR" ;;
        esac
    else
        failed=$((failed + 1))
        printf 'Profile failed: %s (exit %s); inspect %s\n' "$scenario_name" "$status" "$PERF_PROFILE_OUTPUT_DIR" >&2
    fi
done
printf 'Profiles: %s captured, %s failed.\n' "$captured" "$failed" >&2
[ "$failed" -eq 0 ]
