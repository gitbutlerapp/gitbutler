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

git checkout -b destination
echo "change from destination" > shared.txt
git commit -am "destination: conflicting change"
git checkout master
popd

git clone remote-project local-clone
pushd local-clone
git checkout master
target_branch="$(git rev-parse --symbolic-full-name @{u})"
target_branch="${target_branch#refs/remotes/}"
"$BUT" setup
"$BUT" config target "$target_branch"

git branch destination origin/destination
git checkout -b source-lower origin/master
echo "change from source stack" > shared.txt
git commit -am "source-lower: conflicting change"
git checkout -b source-tip
echo "source tip" > source-tip.txt
git add source-tip.txt
git commit -m "source-tip: add file"
git checkout gitbutler/workspace

# Build a two-branch stack, retain it as unapplied metadata, then apply the
# independent branch that it conflicts with.
"$BUT" apply source-lower
"$BUT" apply source-tip
"$BUT" unapply source-tip
"$BUT" apply destination
popd
