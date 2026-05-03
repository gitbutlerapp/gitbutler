#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "DIRECTORY: $1"

# Fetch updates from the remote.
pushd "$1"
  git fetch origin
popd
