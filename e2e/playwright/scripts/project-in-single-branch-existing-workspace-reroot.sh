#!/bin/bash

set -euo pipefail

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

mkdir local-clone
pushd local-clone
git init -b main --object-format=sha1
git commit --allow-empty -m "base"

git checkout main
git checkout -b A
echo "line 1" >> A.txt
git add A.txt
git commit -m "Add A.txt"

git checkout main
git checkout -b B
echo "line 1" >> B.txt
git add B.txt
git commit -m "Add B.txt"

git checkout main
git checkout -b C
echo "line 1" >> C.txt
git add C.txt
git commit -m "Add C.txt"

git checkout main
"$BUT" setup

"$BUT" apply A
"$BUT" apply B
"$BUT" apply C
test "$(git branch --show-current)" = "gitbutler/workspace"

# Leave the managed workspace through one of its already-applied branches. Applying another
# already-applied branch from the UI should rebuild the managed workspace around A and C only.
git switch A
popd
