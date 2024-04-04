#!/usr/bin/env bash
set -euo pipefail

THIS="$0"
function log {
    printf "[%s] %s\n\n" "$THIS" "$*"
}

ROOT="$(dirname "$THIS")/../.."
TARGET_ROOT="$ROOT/target/release"
CRATE_ROOT="$ROOT/crates/gitbutler-tauri"

if [ -f "$TARGET_ROOT/gitbutler-git-askpass" ] && [ -f "$TARGET_ROOT/gitbutler-git-setsid" ]; then
    TRIPLE="$(rustc -vV | sed -n 's|host: ||p')"
    log injecting gitbutler-git binaries into crates/gitbutler-tauri "(TRIPLE=${TRIPLE})"
    cp -v "$TARGET_ROOT/gitbutler-git-askpass" "$CRATE_ROOT/gitbutler-git-askpass-${TRIPLE}"
    cp -v "$TARGET_ROOT/gitbutler-git-setsid" "$CRATE_ROOT/gitbutler-git-setsid-${TRIPLE}"
elif [ -f "$TARGET_ROOT/gitbutler-git-askpass.exe" ] && [ -f "$TARGET_ROOT/gitbutler-git-setsid.exe" ]; then
    TRIPLE="$(rustc.exe -vV | sed -n 's|host: ||p')"
    log injecting gitbutler-git binaries into crates/gitbutler-tauri "(TRIPLE=${TRIPLE})"
    cp -v "$TARGET_ROOT/gitbutler-git-askpass.exe" "$CRATE_ROOT/gitbutler-git-askpass-${TRIPLE}.exe"
    cp -v "$TARGET_ROOT/gitbutler-git-setsid.exe" "$CRATE_ROOT/gitbutler-git-setsid-${TRIPLE}.exe"
else
    log gitbutler-git binaries are not built
    exit 1
fi
