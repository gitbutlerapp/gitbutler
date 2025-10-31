#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $GITBUTLER_CLI_DATA_DIR"
echo "BUT_TESTING $BUT_TESTING"

pushd remote-project
# Checkout branch 1
git checkout branch1
echo "branch1 commit 3" >> a_file
git commit -am "branch1: third commit"

git checkout master
popd

