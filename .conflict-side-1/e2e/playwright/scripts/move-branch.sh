#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"
echo "BRANCH DESTINATION: $1"
echo "BRANCH TO MOVE: $2"
echo "DIRECTORY: $3"

# Move the source branch on top of the target branch.
pushd "$3"
  "$BUT" move "$1" "$2"
popd
