#!/usr/bin/env bash
set -euo pipefail

THIS="$0"
function log {
    printf "[%s] %s\n\n" "$THIS" "$*"
}


if [ -f "target/release/gitbutler-git-askpass" ] && [ -f "target/release/gitbutler-git-setsid" ]; then
    TRIPLE="$(rustc -vV | sed -n 's|host: ||p')"
    log injecting gitbutler-git binaries into gitbutler-app "(TRIPLE=${TRIPLE})"
    cp -v target/release/gitbutler-git-askpass "gitbutler-app/gitbutler-git-askpass-${TRIPLE}"
    cp -v target/release/gitbutler-git-setsid "gitbutler-app/gitbutler-git-setsid-${TRIPLE}"
elif [ -f "target/release/gitbutler-git-askpass.exe" ] && [ -f "target/release/gitbutler-git-setsid.exe" ]; then
    TRIPLE="$(rustc.exe -vV | sed -n 's|host: ||p')"
    log injecting gitbutler-git binaries into gitbutler-app "(TRIPLE=${TRIPLE})"
    cp -v target/release/gitbutler-git-askpass.exe "gitbutler-app/gitbutler-git-askpass-${TRIPLE}.exe"
    cp -v target/release/gitbutler-git-setsid.exe "gitbutler-app/gitbutler-git-setsid-${TRIPLE}.exe"
else
    log gitbutler-git binaries are not built
    exit 1
fi
