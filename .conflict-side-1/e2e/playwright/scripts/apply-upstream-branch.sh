#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $GITBUTLER_CLI_DATA_DIR"
echo "BUT_TESTING $BUT_TESTING"
echo "BRANCH TO APPLY: $1"
echo "DIRECTORY: $2"

# Apply remote branch to the workspace.
pushd "$2"
  $BUT_TESTING -j stack-branches -u -b $1
popd
