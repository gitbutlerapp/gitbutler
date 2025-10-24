#!/usr/bin/env bash
set -euo pipefail

THIS="$0"
function log {
    printf "[%s] %s\n\n" "$THIS" "$*"
}

ROOT="$(dirname "$THIS")/../.."
TRIPLE=${TRIPLE_OVERRIDE:-$(rustc -vV | sed -n 's|host: ||p')}
TARGET_ROOT="${CARGO_TARGET_DIR:-$ROOT/target}/${TRIPLE_OVERRIDE:-}/release"
CRATE_ROOT="$ROOT/crates/gitbutler-tauri"

# BINARIES
ASKPASS="gitbutler-git-askpass"
SETSID="gitbutler-git-setsid"
BUT="but"


if [ -f "$TARGET_ROOT/$ASKPASS" ] && [ -f "$TARGET_ROOT/$SETSID" ] && [ -f "$TARGET_ROOT/$BUT" ]; then
    log injecting gitbutler-git binaries into crates/gitbutler-tauri "(TRIPLE=${TRIPLE})"
    cp -v "$TARGET_ROOT/$ASKPASS" "$CRATE_ROOT/$ASKPASS-${TRIPLE}"
    cp -v "$TARGET_ROOT/$SETSID" "$CRATE_ROOT/$SETSID-${TRIPLE}"
    cp -v "$TARGET_ROOT/$BUT" "$CRATE_ROOT/$BUT-${TRIPLE}"
elif [ -f "$TARGET_ROOT/$ASKPASS.exe" ] && [ -f "$TARGET_ROOT/$SETSID.exe" ] && [ -f "$TARGET_ROOT/$BUT.exe" ]; then
    log injecting gitbutler-git binaries into crates/gitbutler-tauri "(TRIPLE=${TRIPLE})"
    cp -v "$TARGET_ROOT/$ASKPASS.exe" "$CRATE_ROOT/$ASKPASS-${TRIPLE}.exe"
    cp -v "$TARGET_ROOT/$SETSID.exe" "$CRATE_ROOT/$SETSID-${TRIPLE}.exe"
    cp -v "$TARGET_ROOT/$BUT.exe" "$CRATE_ROOT/$BUT-${TRIPLE}.exe"
else
    log gitbutler-git binaries are not built
    exit 1
fi
