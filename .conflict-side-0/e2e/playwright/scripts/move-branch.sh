#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $GITBUTLER_CLI_DATA_DIR"
echo "BUT_TESTING $BUT_TESTING"
echo "BRANCH DESTINATION: $1"
echo "BRANCH TO MOVE: $2"
echo "DIRECTORY: $3"

# Apply remote branch to the workspace.
pushd "$3"
  $BUT_TESTING -j move-branch $1 $2
popd
