#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

pushd remote-project
  git checkout master
  echo "upstream changed this line" > a_file
  git add a_file
  git commit -m "base: change conflicting file"
popd

pushd local-clone
  echo "local dirty change" > a_file
popd
