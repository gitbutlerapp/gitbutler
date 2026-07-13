#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

FORGE_REMOTE_URL="${1:-}"

# Setup a remote project. GitButler currently requires projects to have a remote.
mkdir remote-project
pushd remote-project
git init -b master --object-format=sha1
echo "base line 1" >> a_file
echo "base line 2" >> a_file
echo "base line 3" >> a_file
git add a_file
git commit -m "base: initial commit"
git config http.receivepack true
popd

# Clone the remote, register the project with GitButler, configure the target,
# then leave HEAD on a normal non-target branch before the app opens.
git clone remote-project local-clone
pushd local-clone
  git checkout master
  target_branch="$(git rev-parse --symbolic-full-name @{u})"
  target_branch="${target_branch#refs/remotes/}"
  echo "Target branch: $target_branch"
  "$BUT" setup
  "$BUT" config target "$target_branch"

  git checkout -b single-branch-fixture master
  echo "single branch commit 1" > single_branch_first.txt
  git add single_branch_first.txt
  git commit -m "single-branch: first commit"

  echo "single branch commit 2" > single_branch_second.txt
  git add single_branch_second.txt
  git commit -m "single-branch: second commit"

  echo "single branch file" > single_branch_file.txt
  git add single_branch_file.txt
  git commit -m "single-branch: add file"

  if [ -n "$FORGE_REMOTE_URL" ]; then
    git push -u origin single-branch-fixture
    git remote set-url origin "$FORGE_REMOTE_URL"
  fi
popd
