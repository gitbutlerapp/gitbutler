#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"
echo "PROJECT NAME: $1"
echo "BRANCH NAME: $2"

pushd "$1"
  git checkout -b "$2" "$(git rev-parse refs/remotes/origin/master)"
popd
