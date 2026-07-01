#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

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

git checkout master

# Create branch 2
git checkout -b branch2
echo "branch2 commit 1" >> a_file
git commit -am "branch2: first commit"
echo "branch2 commit 2" >> a_file
git commit -am "branch2: second commit"
git checkout master
popd

# Clone the remote into a folder and add the project to the application.
git clone remote-project local-clone
pushd local-clone
  git checkout master
  target_branch="$(git rev-parse --symbolic-full-name @{u})"
  target_branch="${target_branch#refs/remotes/}"
  "$BUT" setup
  "$BUT" config target "$target_branch"
popd

# Clone the remote into another folder and add the project as well.
git clone remote-project local-clone-2
pushd local-clone-2
  git checkout master
  target_branch="$(git rev-parse --symbolic-full-name @{u})"
  target_branch="${target_branch#refs/remotes/}"
  "$BUT" setup
  "$BUT" config target "$target_branch"
popd
