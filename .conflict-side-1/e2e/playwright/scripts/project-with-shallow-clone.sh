#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

# Setup a remote project with enough history to make shallow clones meaningful.
mkdir remote-project
pushd remote-project
git init -b master --object-format=sha1
echo "initial content" >> a_file
git add a_file
git commit -am "initial commit"

for i in $(seq 1 10); do
  echo "line $i" >> a_file
  git commit -am "commit $i"
done

popd

# Shallow-clone with only the latest commit.
git clone --depth 1 remote-project local-clone
pushd local-clone
  git checkout master
  target_branch="$(git rev-parse --symbolic-full-name @{u})"
  target_branch="${target_branch#refs/remotes/}"
  "$BUT" setup
  "$BUT" config target "$target_branch"
popd
