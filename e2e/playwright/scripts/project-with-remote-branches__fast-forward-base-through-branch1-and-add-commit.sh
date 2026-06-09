#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

pushd remote-project
  git checkout master
  git merge --ff-only branch1
  echo "base advanced after integrated branch" >> x_file
  git add x_file
  git commit -m "base: advanced after integrated branch"
popd
