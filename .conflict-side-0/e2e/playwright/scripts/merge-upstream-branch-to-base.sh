#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $GITBUTLER_CLI_DATA_DIR"
echo "BUT_TESTING $BUT_TESTING"
echo "BRANCH TO APPLY: $1"
# Apply remote branch to the workspace.
pushd remote-project
  git checkout master
  git merge --no-ff -m "Merging upstream branch $1 into base" "$1"
popd
