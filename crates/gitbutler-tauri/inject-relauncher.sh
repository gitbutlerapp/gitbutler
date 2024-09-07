#!/usr/bin/env bash
# Injects the relauncher into the right place with the correct platform
# information. It is only needed for macos, but there is no way to restrict
# "externalBin" in the turi configuration to a specific platform.
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
    cp -v "$CRATE_ROOT/relauncher.sh" "$CRATE_ROOT/$RELAUNCHER-${TRIPLE}"
else
    cp -v "$CRATE_ROOT/relauncher.sh" "$CRATE_ROOT/$RELAUNCHER-${TRIPLE}.exe"
fi
