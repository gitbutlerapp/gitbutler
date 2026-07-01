#!/bin/bash

set -euo pipefail

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

# Start with a remote-backed project because GitButler setup expects one in e2e.
mkdir remote-project
pushd remote-project
git init -b master --object-format=sha1
echo "base" > base.txt
git add base.txt
git commit -m "base: initial commit"

git checkout -b A
echo "A" > A.txt
git add A.txt
git commit -m "A: first commit"

git checkout master
git checkout -b B
echo "B" > B.txt
git add B.txt
git commit -m "B: first commit"

git checkout master
git checkout -b C
echo "C" > C.txt
git add C.txt
git commit -m "C: first commit"

git checkout master
popd

git clone remote-project local-clone
pushd local-clone
git checkout master
target_branch="$(git rev-parse --symbolic-full-name @{u})"
target_branch="${target_branch#refs/remotes/}"
"$BUT" setup
"$BUT" config target "$target_branch"

git branch --track A origin/A
git branch --track B origin/B
git branch --track C origin/C

"$BUT" apply A
"$BUT" apply B
test "$(git branch --show-current)" = "gitbutler/workspace"

# Enter single-branch/ad-hoc mode from a branch already enclosed by the workspace.
git checkout B
popd
