#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"
echo "BRANCH TO SQUASH: $1"

# Squash the upstream branch into master and delete the upstream branch.
pushd remote-project
  git checkout master
  git merge --squash "$1"
  git commit -m "Squashing upstream branch $1 into base"
  git branch -D "$1"
popd
