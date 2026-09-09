#!/bin/sh
# Send a saved envelope unchanged. Also used by run.sh for upload preflight.
# Never trace credentials, including when invoked with sh -x.
set +x
set -eu

upload_die() {
    printf 'performance upload error: %s\n' "$*" >&2
    if [ -n "${payload:-}" ]; then
        printf 'Payload retained: %s\nReplay with upload.sh and this path.\n' "$payload" >&2
    fi
    exit 1
}

[ "$#" -eq 1 ] || upload_die 'usage: upload.sh <payload.json> | --check'
payload=
[ "$1" = --check ] || payload=$1
[ -n "${PERF_UPLOAD_URL:-}" ] && [ -n "${PERF_UPLOAD_TOKEN:-}" ] ||
    upload_die 'set both PERF_UPLOAD_URL and PERF_UPLOAD_TOKEN'
for command in curl jq; do
    command -v "$command" >/dev/null 2>&1 || upload_die "required command not found: $command"
done
# HTTPS only, except loopback for local smoke tests. Reject URL credentials,
# queries, fragments and whitespace. Do not print potentially sensitive input.
printf '%s' "$PERF_UPLOAD_URL" | jq -Rse '
    (test("[[:space:]]") | not) and test("^(https://[^/?#@[:space:]]+|http://(localhost|127\\.0\\.0\\.1|\\[::1\\])(:[0-9]+)?)(/[^?#@[:space:]]*)?$")
' >/dev/null || upload_die 'PERF_UPLOAD_URL must be an HTTPS base URL (HTTP allowed on loopback only)'
cr=$(printf '\r')
case "$PERF_UPLOAD_TOKEN" in
    *"$cr"*|*'
'*) upload_die 'PERF_UPLOAD_TOKEN must not contain newlines' ;;
esac
[ "$1" != --check ] || exit 0
[ -r "$payload" ] || upload_die 'payload is not readable'
case "$payload" in
    /*) ;;
    *) payload=$PWD/$payload ;;
esac
[ "$(wc -c <"$payload")" -le 4194304 ] || upload_die 'payload exceeds 4 MiB limit'
jq -e 'type == "object" and .schemaVersion == 1 and
    (.uploadId | type == "string" and length > 0) and
    (.measurements | type == "array" and length > 0 and length <= 100) and
    ([.measurements[].scenario] | length == (unique | length)) and
    all(.measurements[]; (.hyperfine.results | type == "array" and length == 1) and
        (.hyperfine.results[0].times | type == "array" and length > 0 and length <= 10000)) and
    ([.measurements[].hyperfine.results[0].times | length] | add <= 100000)
' "$payload" >/dev/null 2>&1 || upload_die 'invalid benchmark envelope'

umask 077
upload_tmp=$(mktemp -d "${TMPDIR:-/tmp}/but-performance-upload.XXXXXX") ||
    upload_die 'could not create private upload directory'
trap 'rm -rf "$upload_tmp"' EXIT
trap 'exit 130' INT
trap 'exit 143' HUP TERM
printf 'Authorization: Token %s\nContent-Type: application/json\n' "$PERF_UPLOAD_TOKEN" >"$upload_tmp/headers"
unset PERF_UPLOAD_TOKEN
# Snapshot once so even concurrent edits to the retained file cannot change a retry.
cp "$payload" "$upload_tmp/payload.json" || upload_die 'could not snapshot saved payload'

attempt=1
delay=1
while :; do
    curl_status=0
    # Disable curlrc, never follow redirects, and never put secrets in argv.
    # Response/error bodies aren't logged: an untrusted server may echo credentials.
    http_status=$(curl --disable --silent --show-error \
        --proto '=http,https' --connect-timeout 10 --max-time 35 \
        --request POST --header "@$upload_tmp/headers" \
        --data-binary "@$upload_tmp/payload.json" \
        --output "$upload_tmp/receipt.json" --write-out '%{http_code}' \
        "${PERF_UPLOAD_URL%/}/v1/benchmarks/uploads" 2>"$upload_tmp/curl-error") || curl_status=$?
    if [ "$curl_status" -eq 0 ]; then
        case "$http_status" in
            200|201)
                # Require a JSON receipt, not a proxy's successful HTML response.
                jq -e --argjson duplicate "$([ "$http_status" = 200 ] && printf true || printf false)" '
                    type == "object" and .duplicate == $duplicate and
                    (.sessionId | (type == "string" and length > 0) or
                        (type == "number" and . > 0 and floor == .))
                ' "$upload_tmp/receipt.json" >/dev/null 2>&1 || upload_die 'HTTP success with invalid receipt; replay saved payload to confirm acceptance'
                printf 'Benchmark upload accepted (HTTP %s). Payload: %s\n' "$http_status" "$payload"
                exit 0
                ;;
            429|500|502|503|504) ;;
            401|403) upload_die "HTTP $http_status: check benchmark-suite token and permissions" ;;
            409) upload_die 'HTTP 409: upload ID already exists with different content; restore original payload' ;;
            *) upload_die "HTTP $http_status: request rejected; check URL and payload against receiver contract" ;;
        esac
    fi
    [ "$attempt" -lt 5 ] || upload_die "upload failed after $attempt attempts (curl $curl_status, HTTP $http_status)"
    printf 'Upload attempt %s failed (curl %s, HTTP %s); retrying in %ss.\n' \
        "$attempt" "$curl_status" "$http_status" "$delay" >&2
    sleep "$delay"
    delay=$((delay * 2))
    attempt=$((attempt + 1))
done
