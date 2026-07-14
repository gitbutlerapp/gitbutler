#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"
echo "BRANCH TO CHECKOUT: $1"
echo "DIRECTORY: $2"

# Simulate an external `git checkout` (e.g. from a terminal) of a branch.
pushd "$2"
  git checkout "$1"
popd
