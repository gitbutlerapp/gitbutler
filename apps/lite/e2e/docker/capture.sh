#!/usr/bin/env bash
# Capture one side of a before/after comparison in the container.
#
#   apps/lite/e2e/docker/capture.sh before
#   apps/lite/e2e/docker/capture.sh after
#
# Arguments after the name go to Playwright:
#
#   apps/lite/e2e/docker/capture.sh proof --retries=0 --grep "workspace sidebar"
#
# Writes apps/lite/e2e/screenshots/<name>/, which is what compare-screenshots.mjs
# reads. The host checkout is only ever read, so the branch can be unapplied and
# re-applied between the two calls without the container noticing.
set -euo pipefail

name="${1:-}"
if [ -z "$name" ]; then
	echo "usage: capture.sh <output-directory-name> [playwright args...]" >&2
	exit 1
fi
shift

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
image="gitbutler/lite-screenshots:latest"

echo "==> building $image"
docker build --tag "$image" "$repo_root/apps/lite/e2e/docker"

# The whole screenshots directory is mounted, not the single output one: the
# spec clears its own output directory before capturing, and rm on a mount point
# fails with EBUSY.
screenshots_dir="$repo_root/apps/lite/e2e/screenshots"
mkdir -p "$screenshots_dir"

# A signal sent to this script does not reach `docker run` — the shell dies, the
# container keeps running, and whatever it had written is never flushed. Under an
# outer timeout that loses exactly the diagnostics the run was for, so stop the
# container explicitly and give it time to finish writing.
container="lite-screenshots-$name"
cleanup() { docker stop --time 20 "$container" > /dev/null 2>&1 || true; }
trap cleanup INT TERM EXIT

# --shm-size: Chromium maps shared memory per renderer and the 64MB default
#   crashes it under load, which reads as a renderer that never paints — the
#   exact symptom this image exists to rule out.
# --init: reaps the Electron and Xvfb children a killed run leaves behind.
echo "==> capturing '$name'"
docker run --rm --init \
	--name "$container" \
	--shm-size=2g \
	--volume "$repo_root:/src:ro" \
	--volume "$screenshots_dir:/work/apps/lite/e2e/screenshots" \
	--volume gitbutler-lite-screenshots-work:/work \
	--volume gitbutler-lite-screenshots-cargo:/cargo-target \
	--volume gitbutler-lite-screenshots-pnpm:/pnpm-store \
	--env "SCREENSHOT_OUT=$name" \
	--env DEBUG \
	"$image" "$@"

echo "==> wrote $screenshots_dir/$name"
