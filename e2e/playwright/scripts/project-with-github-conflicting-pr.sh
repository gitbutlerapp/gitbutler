#!/bin/bash

set -euo pipefail

FORGE_REMOTE_URL="${1:?forge remote URL is required}"

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

mkdir remote-project
pushd remote-project
git init -b master --object-format=sha1
echo "base" > shared.txt
git add shared.txt
git commit -m "base commit"

git checkout -b applied-feature
echo "change from applied branch" > shared.txt
git commit -am "applied-feature: conflicting change"

git checkout master
popd

git clone remote-project fork-project
pushd fork-project
git checkout -b fork-feature
echo "change from pull request" > shared.txt
git commit -am "fork-feature: conflicting change"
popd

git clone --bare fork-project fork-project-bare

git clone remote-project local-clone
pushd local-clone
git checkout master
target_branch="$(git rev-parse --symbolic-full-name @{u})"
target_branch="${target_branch#refs/remotes/}"
"$BUT" setup
"$BUT" config target "$target_branch"
git remote set-url origin "$FORGE_REMOTE_URL"
popd
