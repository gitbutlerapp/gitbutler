#!/bin/bash

set -euo pipefail

FORGE_REMOTE_URL="${1:?forge remote URL is required}"

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

mkdir remote-project
pushd remote-project
git init -b master --object-format=sha1
echo "base" > base.txt
git add base.txt
git commit -m "base commit"

git checkout -b base-pr-feature
echo "base pr content" > base-pr.txt
git add base-pr.txt
git commit -m "base-pr: add pr content"

git checkout master
git checkout -b single-branch-fixture
echo "single branch content" > single-branch.txt
git add single-branch.txt
git commit -m "single-branch: add content"

git checkout master
popd

git clone remote-project fork-project
pushd fork-project
git checkout -b fork-feature
echo "fork pr content" > fork-pr.txt
git add fork-pr.txt
git commit -m "fork: add pr content"
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
