#!/bin/bash

set -euo pipefail

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

mkdir remote-project
pushd remote-project
git init -b master --object-format=sha1
echo "base" > shared.txt
git add shared.txt
git commit -m "base: initial commit"

git checkout -b branch-a
echo "change from branch A" > shared.txt
git commit -am "branch-a: conflicting change"

git checkout master
git checkout -b branch-b
echo "change from branch B" > shared.txt
git commit -am "branch-b: conflicting change"
git checkout master
popd

git clone remote-project local-clone
pushd local-clone
git checkout master
target_branch="$(git rev-parse --symbolic-full-name @{u})"
target_branch="${target_branch#refs/remotes/}"
"$BUT" setup
"$BUT" config target "$target_branch"
git checkout -b branch-a origin/branch-a
git branch -D gitbutler/workspace
popd
