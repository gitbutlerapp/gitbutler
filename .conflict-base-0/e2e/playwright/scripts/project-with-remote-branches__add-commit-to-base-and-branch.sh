#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

pushd remote-project
# Checkout branch 1
git checkout master
echo "create file b" >> b_file
git add b_file
git commit -am "commit in base"

git checkout branch1
git rebase master
echo "update file b" >> b_file
git add b_file
git commit -am "branch1: update after base change"

git checkout master
popd
