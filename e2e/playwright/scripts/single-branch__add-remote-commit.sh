#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

pushd remote-project
  git checkout single-branch-fixture
  echo "remote single branch content" > remote_single_branch_file.txt
  git add remote_single_branch_file.txt
  git commit -m "single-branch: remote commit"
  git checkout master
popd
