#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

pushd remote-project
  git checkout single-branch-fixture
  echo "remote conflict content" > single_branch_file.txt
  git commit -am "single-branch: remote conflicting commit"
  git checkout master
popd
