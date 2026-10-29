#!/bin/sh
# Discover saved benchmarks and optional captures. Receipts live only in memory.
# Never trace credentials, including when invoked with sh -x.
set +x
set -eu
export LC_ALL=C

PERF_ROOT=$(CDPATH='' cd "$(dirname "$0")" && pwd)
# shellcheck disable=SC1091
. "$PERF_ROOT/lib.sh"

upload_die() {
    printf 'performance upload error: %s\n' "$*" >&2
    printf 'Retry upload.sh with the unchanged results directory.\n' >&2
    exit 1
}

[ "$#" -eq 1 ] || upload_die 'usage: upload.sh <PERF_RESULTS_DIR>'
[ -d "$1" ] || upload_die 'results directory does not exist'
results_dir=$(CDPATH='' cd "$1" && pwd)
[ -n "${PERF_UPLOAD_URL:-}" ] && [ -n "${PERF_UPLOAD_TOKEN:-}" ] ||
    upload_die 'set both PERF_UPLOAD_URL and PERF_UPLOAD_TOKEN'
for tool in curl jq; do perf_require_command "$tool"; done
# HTTPS only, except loopback. Do not print potentially sensitive input.
printf '%s' "$PERF_UPLOAD_URL" | jq -Rse '
    (test("[[:space:]]") | not) and test("^(https://[^/?#@[:space:]]+|http://(localhost|127\\.0\\.0\\.1|\\[::1\\])(:[0-9]+)?)(/[^?#@[:space:]]*)?$")
' >/dev/null || upload_die 'PERF_UPLOAD_URL must be an HTTPS base URL (HTTP allowed on loopback only)'
cr=$(printf '\r')
case "$PERF_UPLOAD_TOKEN" in
    *"$cr"*|*'
'*) upload_die 'PERF_UPLOAD_TOKEN must not contain newlines' ;;
esac

umask 077
upload_tmp=$(mktemp -d "${TMPDIR:-/tmp}/but-performance-upload.XXXXXX") || upload_die 'could not create private upload directory'
trap 'rm -rf "$upload_tmp"' EXIT
trap 'exit 130' INT
trap 'exit 143' HUP TERM
printf 'Authorization: Token %s\nContent-Type: application/json\n' "$PERF_UPLOAD_TOKEN" >"$upload_tmp/headers"
unset PERF_UPLOAD_TOKEN
[ -s "$results_dir/metadata.json" ] || upload_die 'missing run metadata.json'
cp "$results_dir/metadata.json" "$upload_tmp/metadata.json"
: >"$upload_tmp/measurements.jsonl"
: >"$upload_tmp/scenarios"
# Visible directories are scenario names; hidden .profiling holds local diagnostics.
for scenario_dir in "$results_dir"/*; do
    [ -d "$scenario_dir" ] || continue
    scenario=$(basename "$scenario_dir")
    case "$scenario" in
        ''|.*|-*|*[!a-zA-Z0-9_-]*) upload_die 'unsafe scenario directory name' ;;
    esac
    benchmark=$scenario_dir/benchmark.json
    [ -s "$benchmark" ] || upload_die "missing benchmark.json for $scenario; profiles require a benchmark"
    [ "$(wc -c <"$benchmark")" -le 4194304 ] || upload_die "benchmark exceeds 4 MiB: $scenario"
    jq -cn --arg scenario "$scenario" --slurpfile hyperfine "$benchmark" '
        if ($hyperfine | length) != 1 or ($hyperfine[0].results | length) != 1 then
            error("expected one Hyperfine document with one result")
        else {scenario: $scenario, hyperfine: $hyperfine[0]} end
    ' >>"$upload_tmp/measurements.jsonl" || upload_die "invalid benchmark: $scenario"
    printf '%s\n' "$scenario" >>"$upload_tmp/scenarios"
done
# Canonical ordering gives byte-identical benchmark replay, even after adding profiles.
jq -S --slurpfile measurements "$upload_tmp/measurements.jsonl" '
    . + {measurements: $measurements}
' "$upload_tmp/metadata.json" >"$upload_tmp/payload.json" || upload_die 'invalid run metadata'
[ "$(wc -c <"$upload_tmp/payload.json")" -le 4194304 ] || upload_die 'payload exceeds 4 MiB limit'
jq -se 'length == 1 and (.[0] |
    type == "object" and .schemaVersion == 1 and
    (.uploadId | type == "string" and length > 0) and
    (.binary.commitSha | type == "string" and test("^[0-9a-f]{40}$")) and
    (.measurements | type == "array" and length > 0 and length <= 100) and
    all(.measurements[]; (.hyperfine.results | type == "array" and length == 1) and
        (.hyperfine.results[0].times | type == "array" and length > 0 and length <= 10000)) and
    ([.measurements[].hyperfine.results[0].times | length] | add <= 100000))
' "$upload_tmp/payload.json" >/dev/null 2>&1 || upload_die 'invalid benchmark envelope'

# Snapshot each body once. All transport retries use identical bytes; stdout is
# reserved for the receipt so callers retain it in a shell variable, never a file.
upload_json() (
    body=$1
    endpoint=$2
    kind=$3
    cp "$body" "$upload_tmp/body.json" || upload_die 'could not snapshot upload artifact'
    if [ "$kind" = profile ]; then perf_validate_profile "$upload_tmp/body.json" || exit 1; fi
    attempt=1
    delay=1
    newline='
'
    while :; do
        curl_status=0
        response=$(curl --disable --silent --show-error \
            --proto '=http,https' --connect-timeout 10 --max-time 35 \
            --request POST --header "@$upload_tmp/headers" \
            --data-binary "@$upload_tmp/body.json" --write-out '\n%{http_code}' \
            "${PERF_UPLOAD_URL%/}$endpoint" 2>"$upload_tmp/curl-error") || curl_status=$?
        http_status=${response##*"$newline"}
        response=${response%"$newline"*}
        if [ "$curl_status" -eq 0 ]; then
            case "$http_status" in
                200|201)
                    printf '%s' "$response" | jq -se --arg kind "$kind" \
                        --argjson duplicate "$([ "$http_status" = 200 ] && printf true || printf false)" '
                        def positive_id: (type == "number" and . > 0 and floor == .);
                        length == 1 and (.[0] | type == "object" and .duplicate == $duplicate and
                        if $kind == "benchmark" then
                            (.sessionId | (type == "string" and length > 0) or positive_id)
                        else (.id | positive_id) end)
                    ' >/dev/null 2>&1 || upload_die 'HTTP success with invalid receipt'
                    printf '%s upload accepted (HTTP %s).\n' "$kind" "$http_status" >&2
                    printf '%s' "$response"
                    exit 0
                    ;;
                429|5[0-9][0-9]) ;;
                401|403) upload_die "HTTP $http_status: check benchmark-suite token and permissions" ;;
                409) upload_die 'HTTP 409: conflicting content/identity or profile cap; do not rename or regenerate artifacts' ;;
                *) upload_die "HTTP $http_status: request rejected; check URL and artifact against receiver contract" ;;
            esac
        fi
        [ "$attempt" -lt 5 ] || upload_die "upload failed after $attempt attempts (curl $curl_status, HTTP $http_status)"
        printf 'Upload attempt %s failed (curl %s, HTTP %s); retrying in %ss.\n' \
            "$attempt" "$curl_status" "$http_status" "$delay" >&2
        sleep "$delay"
        delay=$((delay * 2))
        attempt=$((attempt + 1))
    done
)

receipt=$(upload_json "$upload_tmp/payload.json" /v1/benchmarks/uploads benchmark)
# Never trust array position or a response from another execution.
printf '%s' "$receipt" | jq -e --slurpfile payload "$upload_tmp/payload.json" '
    .uploadId == $payload[0].uploadId and
    (.measurements | type == "array") and
    all(.measurements[]; (.id | type == "number" and . > 0 and floor == .)) and
    ([.measurements[].id] | length == (unique | length)) and
    ([.measurements[].scenario] | sort) == ([$payload[0].measurements[].scenario] | sort)
' >/dev/null 2>&1 || upload_die 'missing or mismatched scenario receipt'

failed=0
attached=0
while IFS= read -r scenario; do
    scenario_dir=$results_dir/$scenario
    profile=$scenario_dir/profile.json
    # No capture attempted: benchmark-only results are complete.
    [ -e "$profile" ] || [ -e "$scenario_dir/profile-status" ] || continue
    if [ ! -f "$scenario_dir/profile-status" ] || [ "$(cat "$scenario_dir/profile-status")" != 0 ]; then
        printf 'Profile unavailable or recording failed: %s\n' "$scenario" >&2
        failed=$((failed + 1))
        continue
    fi
    if ! jq -e --arg scenario "$scenario" --slurpfile metadata "$upload_tmp/metadata.json" '
        .uploadId == $metadata[0].uploadId and .binarySha == $metadata[0].binary.commitSha and
        .scenario == $scenario and .backend == "samply-presymbolicate"
    ' "$scenario_dir/profile-metadata.json" >/dev/null 2>&1; then
        printf 'Profile provenance does not match benchmark: %s\n' "$scenario" >&2
        failed=$((failed + 1))
        continue
    fi
    measurement_id=$(printf '%s' "$receipt" | jq -er --arg scenario "$scenario" '
        .measurements[] | select(.scenario == $scenario) | .id
    ')
    if upload_json "$profile" "/v1/benchmarks/measurements/$measurement_id/profiles?name=main" profile >/dev/null; then
        attached=$((attached + 1))
    else
        failed=$((failed + 1))
    fi
done <"$upload_tmp/scenarios"
printf 'Profile attachments: %s accepted, %s failed.\n' "$attached" "$failed" >&2
[ "$failed" -eq 0 ]
