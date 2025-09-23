#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $GITBUTLER_CLI_DATA_DIR"
echo "BUT_TESTING $BUT_TESTING"

# Setup a remote project.
# GitButler currently requires projects to have a remote
mkdir remote-project
pushd remote-project
git init -b master --object-format=sha1
echo "foo" >> a_file
echo "bar" >> a_file
echo "baz" >> a_file
git add a_file
git commit -am "Hey, look! A commit."

# Create branch 1
git checkout -b branch1
echo "branch1 commit 1" >> a_file
git commit -am "branch1: first commit"
echo "branch1 commit 2" >> a_file
git commit -am "branch1: second commit"
echo "branch1 commit 1" >> a_file
git commit -am "branch1: third commit"
echo "branch1 commit 2" >> a_file
git commit -am "branch1: fourth commit"
git checkout master

# Create branch 2
# Branch 2 is independent
git checkout -b branch2
echo "branch2 commit 1" >> b_file
git add b_file
git commit -am "branch2: first commit"
echo "branch2 commit 2" >> b_file
git commit -am "branch2: second commit"
git checkout master

# Create branch 3, also independent
git checkout -b branch3
echo "branch3 commit 1" >> c_file
git add c_file
git commit -am "branch3: first commit"
echo "branch3 commit 2" >> c_file
git commit -am "branch3: second commit"
git checkout master
popd

# Clone the remote into a folder and add the project to the application.
git clone remote-project local-clone
pushd local-clone
  git checkout master
  $BUT_TESTING add-project --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"
popd
