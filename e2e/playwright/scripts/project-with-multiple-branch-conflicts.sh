#!/bin/bash

set -euo pipefail

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

mkdir remote-project
pushd remote-project
git init -b master --object-format=sha1
cat > shared.txt <<EOF
slot-a
context-1
context-2
context-3
context-4
context-5
slot-c
EOF
git add shared.txt
git commit -m "base: initial commit"

git checkout -b branch-a
sed -i.bak 's/slot-a/change from branch A/' shared.txt
rm shared.txt.bak
git commit -am "branch-a: change first slot"

git checkout master
git checkout -b branch-c
sed -i.bak 's/slot-c/change from branch C/' shared.txt
rm shared.txt.bak
git commit -am "branch-c: change last slot"

git checkout master
git checkout -b branch-b
sed -i.bak \
  -e 's/slot-a/conflicting change from branch B/' \
  -e 's/slot-c/another conflicting change from branch B/' \
  shared.txt
rm shared.txt.bak
git commit -am "branch-b: conflict with both applied branches"
git checkout master
popd

git clone remote-project local-clone
pushd local-clone
git checkout master
target_branch="$(git rev-parse --symbolic-full-name @{u})"
target_branch="${target_branch#refs/remotes/}"
"$BUT" setup
"$BUT" config target "$target_branch"
popd
