#!/usr/bin/env bash
set -euo pipefail

THIS="$0"
RELAUNCHER="$1"
function log {
    printf "[%s] %s\n\n" "$THIS" "$*"
}

ROOT="$(dirname "$THIS")/../.."
TRIPLE=${TRIPLE_OVERRIDE:-$(rustc -vV | sed -n 's|host: ||p')}
TARGET_ROOT="$ROOT/target/${TRIPLE_OVERRIDE:-}/release"
CRATE_ROOT="$ROOT/crates/gitbutler-tauri"

log injecting relauncher into crates/gitbutler-tauri "(TRIPLE=${TRIPLE})"
if ! [[ $TRIPLE =~ "windows" ]]; then
    cp -v "$CRATE_ROOT/files/relauncher" "$CRATE_ROOT/$RELAUNCHER-${TRIPLE}"
else
    cp -v "$CRATE_ROOT/files/relauncher" "$CRATE_ROOT/$RELAUNCHER-${TRIPLE}.exe"
fi
