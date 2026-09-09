#!/bin/bash

set -euo pipefail

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

branch_count="${1:-12}"

# Setup a remote project. GitButler currently requires projects to have a remote.
mkdir remote-project
pushd remote-project
git init -b master --object-format=sha1
echo "base" > base.txt
git add base.txt
git commit -m "base: initial commit"
popd

# Clone the remote, then give it enough local branches that the branches view is
# taller than the panel showing it and has to scroll. They are made before the
# workspace exists and never applied to it, so they all land in that view. The
# view's default filter is local-only, which is why these are not remote
# branches.
git clone remote-project local-clone
pushd local-clone
git checkout master

for index in $(seq 1 "$branch_count"); do
  git checkout -b "branch-$index" master
  echo "branch $index" > "branch-$index.txt"
  git add "branch-$index.txt"
  git commit -m "branch-$index: first commit"
  echo "branch $index again" >> "branch-$index.txt"
  git commit -am "branch-$index: second commit"
done

git checkout master
target_branch="$(git rev-parse --symbolic-full-name @{u})"
target_branch="${target_branch#refs/remotes/}"
"$BUT" setup
"$BUT" config target "$target_branch"
popd
