#!/usr/bin/env bash
set -euo pipefail

THIS="$0"
function log {
    printf "[%s] %s\n\n" "$THIS" "$*"
}

if [ ! -f "target/release/gitbutler-git-askpass" ] || [ ! -f "target/release/gitbutler-git-setsid" ]; then
    log gitbutler-git binaries are not built
    exit 1
fi

TRIPLE="$(rustc -vV | sed -n 's|host: ||p')"

log injecting gitbutler-git binaries into gitbutler-app "(TRIPLE=${TRIPLE})"
cp -v target/release/gitbutler-git-askpass "gitbutler-app/gitbutler-git-askpass-${TRIPLE}"
cp -v target/release/gitbutler-git-setsid "gitbutler-app/gitbutler-git-setsid-${TRIPLE}"
