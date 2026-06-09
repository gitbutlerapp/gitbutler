#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

pushd remote-project
  git checkout master
  echo "remote target conflict content" > a_file
  git add a_file
  git commit -m "base: conflicting target commit"
popd
