#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"
echo "BRANCH TO MERGE: $1"

# Merge the upstream branch into master and delete the upstream branch
pushd remote-project
  git checkout master
  git merge --no-ff -m "Merging upstream branch $1 into base" "$1"
  git branch -d "$1"
popd
