#!/bin/bash

# Setup a project where a stack ref has been moved below the target base,
# simulating external ref corruption that triggers workspace divergence detection.
#
# This reproduces the production bug: "Branch cannot be created: target commit
# already belongs to another branch."

set -euo pipefail

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

# Create a remote repo with some history.
mkdir remote-project
pushd remote-project
git init -b master --object-format=sha1
echo "initial content" >> a_file
git add a_file
git commit -m "Initial commit"
git checkout master
popd

# Clone it and set up GitButler workspace.
git clone remote-project local-clone
pushd local-clone
  git checkout master
  target_branch="$(git rev-parse --symbolic-full-name @{u})"
  target_branch="${target_branch#refs/remotes/}"
  "$BUT" setup
  "$BUT" config target "$target_branch"
popd
