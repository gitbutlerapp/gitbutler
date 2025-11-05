#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $GITBUTLER_CLI_DATA_DIR"
echo "BUT_TESTING $BUT_TESTING"

# Setup a remote project.
# GitButler currently requires projects to have a remote
mkdir remote-with-changes
pushd remote-with-changes
git init -b master --object-format=sha1
echo "Initial content" >> initial_file.txt
git add initial_file.txt
git commit -am "Initial commit"
popd

# Clone the remote into a folder
git clone remote-with-changes local-with-changes
pushd local-with-changes
  git checkout master

  # Add an extra commit on the main branch
  echo "Second commit content" >> second_file.txt
  git add second_file.txt
  git commit -am "Second commit on main branch"

  # Add some uncommitted changes
  echo "Uncommitted changes" >> uncommitted.txt
  echo "Modified initial file" >> initial_file.txt
popd