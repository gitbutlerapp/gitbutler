#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $GITBUTLER_CLI_DATA_DIR"
echo "BUT_TESTING $BUT_TESTING"

pushd remote-project
# Checkout branch 1
git checkout master
echo "Update to main branch" >> a_file
git add b_file
git commit -am "commit in base"

popd

