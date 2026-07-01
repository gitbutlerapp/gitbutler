#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"
echo "BRANCH TO APPLY: $1"
echo "DIRECTORY: $2"

# Apply remote branch to the workspace.
pushd "$2"
  "$BUT" apply "$1"
popd
