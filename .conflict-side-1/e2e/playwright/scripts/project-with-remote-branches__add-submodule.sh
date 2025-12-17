#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $GITBUTLER_CLI_DATA_DIR"
echo "BUT_TESTING $BUT_TESTING"

# Create a simple repository to use as a submodule
mkdir submodule-repo
pushd submodule-repo
git init -b main --object-format=sha1
echo "submodule content" >> submodule_file
git add submodule_file
git commit -m "Initial submodule commit"
popd

# Add a submodule to the local clone
pushd local-clone
  git submodule add ../submodule-repo my-submodule
popd

