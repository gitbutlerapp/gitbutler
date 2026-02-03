#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $GITBUTLER_CLI_DATA_DIR"
echo "BUT_TESTING $BUT_TESTING"

# Setup a remote project with a single commit.
mkdir remote-with-branch
pushd remote-with-branch
git init -b master --object-format=sha1
echo "foo" >> a_file
git add a_file
git commit -am "Initial commit"
popd

# Clone the remote and add the project to GitButler.
# This creates a workspace with an empty branch (no applied stacks).
git clone remote-with-branch local-clone
pushd local-clone
  git checkout master
  $BUT_TESTING add-project --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"
popd
