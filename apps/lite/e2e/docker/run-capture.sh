#!/usr/bin/env bash
# Container entrypoint: mirror the checkout, build, capture one side of a
# comparison. Invoked by capture.sh, not by hand.
#
# This runs as root and drops to $RUN_AS_USER for every step that does real work,
# rather than dropping once at the top. Two things need root and they happen at
# different times: volume ownership, before anything is installed, and Chromium's
# setuid sandbox helper, which does not exist until after the install.
set -euo pipefail

: "${SCREENSHOT_OUT:?SCREENSHOT_OUT must name an output directory}"
: "${RUN_AS_USER:?RUN_AS_USER must name the unprivileged capture user}"

as_user() {
	setpriv --reuid "$RUN_AS_USER" --regid "$RUN_AS_USER" --init-groups \
		env "HOME=/home/$RUN_AS_USER" "$@"
}

# Electron aborts a root process whose sandbox is still enabled, and waiving the
# sandbox is not an option either — see the switch list in
# playwright.docker.config.ts. So the capture runs as an ordinary user. Docker
# creates volumes owned by root, so ownership is settled here first.
for dir in /work /cargo-target /pnpm-store; do
	mkdir -p "$dir"
	# Only the first run pays for this. Afterwards the files already belong to the
	# capture user, and a recursive chown across node_modules and a cargo target
	# directory is not cheap.
	[ "$(stat -c %U "$dir")" = "$RUN_AS_USER" ] || chown -R "$RUN_AS_USER" "$dir"
done
# The output directory is a fresh bind mount on every run, so it arrives with the
# host's ownership and is missed by the guard above. It holds a handful of PNGs,
# so doing it unconditionally costs nothing.
chown -R "$RUN_AS_USER" /work/apps/lite/e2e/screenshots 2>/dev/null || true

# The host checkout is mounted read-only at /src and mirrored into /work rather
# than built in place. A macOS checkout carries a macOS Electron and macOS native
# modules under node_modules, plus a macOS target/; installing or building over
# any of them would leave the developer's own tree unusable.
#
# --delete keeps a file deleted on the branch from surviving into the capture.
# Excluded paths are not deleted by it, which is what protects the volume's
# node_modules and the mounted output directory.
echo "==> mirroring /src into /work"
as_user rsync -a --delete \
	--exclude ".git/" \
	--exclude "node_modules/" \
	--exclude "target/" \
	--exclude "release/" \
	--exclude "apps/lite/e2e/screenshots/" \
	--exclude "*.node" \
	/src/ /work/

cd /work

echo "==> pnpm install"
as_user pnpm install --frozen-lockfile

# Chromium refuses to start if its setuid helper is present but not setuid root,
# and pnpm has just installed it owned by the capture user. Fixing it here rather
# than passing --no-sandbox is deliberate: a waived sandbox leaves renderers
# created by a reload without startup data, and every surface this catalogue
# reaches by reloading then fails.
sandbox_helper="$(find /work/node_modules/.pnpm -maxdepth 6 -path '*/electron/dist/chrome-sandbox' -print -quit)"
if [ -z "$sandbox_helper" ]; then
	echo "could not find chrome-sandbox under /work/node_modules/.pnpm" >&2
	exit 1
fi
echo "==> configuring $sandbox_helper"
chown root:root "$sandbox_helper"
chmod 4755 "$sandbox_helper"

# The napi SDK is a compiled artefact, and the host's is a macOS one — which is
# why "*.node" is excluded above rather than mirrored. Rebuilding here is what
# puts a Linux one in its place. Cached in /cargo-target, so only the first run
# pays for it.
echo "==> building @gitbutler/but-sdk"
as_user pnpm --filter @gitbutler/but-sdk run build

echo "==> building but"
as_user cargo build -p but

echo "==> building Lite's electron bundle"
as_user pnpm --filter @gitbutler/lite run prepare-askpass --debug
as_user pnpm --filter @gitbutler/lite run build:electron

# A real X server rather than Electron's own headless mode: headless produces no
# compositor frames, and the catalogue captures over CDP from what is on screen.
#
# Trailing arguments go to Playwright, so a caller can narrow the run or shorten
# the budget without editing the config — which is how a hang gets diagnosed.
echo "==> capturing into apps/lite/e2e/screenshots/${SCREENSHOT_OUT}"
as_user xvfb-run --auto-servernum --server-args="-screen 0 ${XVFB_SCREEN}" \
	pnpm --filter @gitbutler/lite exec \
	playwright test --config ./e2e/playwright.docker.config.ts "$@"
