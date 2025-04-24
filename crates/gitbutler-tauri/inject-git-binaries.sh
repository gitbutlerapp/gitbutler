#!/usr/bin/env bash
set -euo pipefail

THIS="$0"
function log {
    printf "[%s] %s\n\n" "$THIS" "$*"
}

ROOT="$(dirname "$THIS")/../.."
TRIPLE=${TRIPLE_OVERRIDE:-$(rustc -vV | sed -n 's|host: ||p')}
TARGET_ROOT="$ROOT/target/${TRIPLE_OVERRIDE:-}/release"
CRATE_ROOT="$ROOT/crates/gitbutler-tauri"

if [ -f "$TARGET_ROOT/gitbutler-git-askpass" ] && [ -f "$TARGET_ROOT/gitbutler-git-setsid" ]; then
    log injecting gitbutler-git binaries into crates/gitbutler-tauri "(TRIPLE=${TRIPLE})"
    cp -v "$TARGET_ROOT/gitbutler-git-askpass" "$CRATE_ROOT/gitbutler-git-askpass-${TRIPLE}"
    cp -v "$TARGET_ROOT/gitbutler-git-setsid" "$CRATE_ROOT/gitbutler-git-setsid-${TRIPLE}"
elif [ -f "$TARGET_ROOT/gitbutler-git-askpass.exe" ] && [ -f "$TARGET_ROOT/gitbutler-git-setsid.exe" ]; then
    log injecting gitbutler-git binaries into crates/gitbutler-tauri "(TRIPLE=${TRIPLE})"
    cp -v "$TARGET_ROOT/gitbutler-git-askpass.exe" "$CRATE_ROOT/gitbutler-git-askpass-${TRIPLE}.exe"
    cp -v "$TARGET_ROOT/gitbutler-git-setsid.exe" "$CRATE_ROOT/gitbutler-git-setsid-${TRIPLE}.exe"
else
    log gitbutler-git binaries are not built
    exit 1
fi
