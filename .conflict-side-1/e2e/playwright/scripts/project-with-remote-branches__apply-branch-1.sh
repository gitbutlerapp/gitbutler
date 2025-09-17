#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $GITBUTLER_CLI_DATA_DIR"
echo "BUT_TESTING $BUT_TESTING"

# Apply branch 1 to the workspace.
pushd local-clone
  $BUT_TESTING stack-branches -u -b branch1
popd
