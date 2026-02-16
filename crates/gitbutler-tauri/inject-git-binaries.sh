#!/usr/bin/env bash
set -euo pipefail

THIS="$0"
function log {
    printf "[%s] %s\n\n" "$THIS" "$*"
}

ROOT="$(dirname "$THIS")/../.."

# Use CARGO_BUILD_TARGET if set, otherwise use rustc default triple
if [ -n "${CARGO_BUILD_TARGET:-}" ]; then
    TRIPLE="$CARGO_BUILD_TARGET"
else
    TRIPLE=${TRIPLE_OVERRIDE:-$(rustc --print host-tuple)}
fi

TARGET_ROOT="${CARGO_TARGET_DIR:-$ROOT/target}/${TRIPLE_OVERRIDE:-${CARGO_BUILD_TARGET:+$CARGO_BUILD_TARGET}}/release"
CRATE_ROOT="$ROOT/crates/gitbutler-tauri"

# BINARIES
ASKPASS="gitbutler-git-askpass"
SETSID="gitbutler-git-setsid"
BUT="but"


if [ -f "$TARGET_ROOT/$ASKPASS.exe" ] && [ -f "$TARGET_ROOT/$SETSID.exe" ]; then
    log injecting windows gitbutler-git binaries into crates/gitbutler-tauri "(TRIPLE=${TRIPLE})"
    cp -v "$TARGET_ROOT/$ASKPASS.exe" "$CRATE_ROOT/$ASKPASS-${TRIPLE}.exe"
    cp -v "$TARGET_ROOT/$SETSID.exe" "$CRATE_ROOT/$SETSID-${TRIPLE}.exe"
    if [ -f "$TARGET_ROOT/$BUT.exe" ]; then
      cp -v "$TARGET_ROOT/$BUT.exe" "$CRATE_ROOT/$BUT-${TRIPLE}.exe"
    fi
elif [ -f "$TARGET_ROOT/$ASKPASS" ] && [ -f "$TARGET_ROOT/$SETSID" ]; then
    log injecting gitbutler-git binaries into crates/gitbutler-tauri "(TRIPLE=${TRIPLE})"
    cp -v "$TARGET_ROOT/$ASKPASS" "$CRATE_ROOT/$ASKPASS-${TRIPLE}"
    cp -v "$TARGET_ROOT/$SETSID" "$CRATE_ROOT/$SETSID-${TRIPLE}"
else
    log gitbutler-git binaries are not built
    exit 1
fi
