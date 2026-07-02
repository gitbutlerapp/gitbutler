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
git checkout -b feature-foo
echo "line 1" >> foo.txt
git add foo.txt
git commit -m "Add foo.txt"

git checkout main
git checkout -b bug-fix
echo "line 1" >> fix.txt
git add fix.txt
git commit -m "Add fix.txt"

git checkout main
"$BUT" setup

"$BUT" apply feature-foo
"$BUT" apply bug-fix
test "$(git branch --show-current)" = "gitbutler/workspace"

# Enter single-branch/ad-hoc mode from one of the branches already enclosed by
# the managed workspace. Applying feature-foo from the UI should return to the
# managed workspace and keep both branches applied.
git switch bug-fix
popd
