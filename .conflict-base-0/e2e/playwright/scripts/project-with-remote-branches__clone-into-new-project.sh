#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $GITBUTLER_CLI_DATA_DIR"
echo "BUT_TESTING $BUT_TESTING"
echo "PROJECT PATH: $1"

# Clone the remote into a folder and add the project to the application.
git clone remote-project $1
pushd "$1"
  git checkout master
  $BUT_TESTING add-project --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"
popd
