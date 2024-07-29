#!/usr/bin/env bash
set -eu -o pipefail
CLI=${1:?The first argument is the GitButler CLI}

git init remote
(cd remote
  echo first > file
  git add . && git commit -m "init"
)

git clone remote single-branch-no-vbranch

git clone remote single-branch-no-vbranch-one-commit
(cd single-branch-no-vbranch-one-commit
  echo change >> file && git add . && git commit -m "local change"
)

git clone remote single-branch-no-vbranch-multi-remote
(cd single-branch-no-vbranch-multi-remote
  git remote add other-origin ../remote
  git fetch other-origin
)

export GITBUTLER_CLI_DATA_DIR=./git/gitbutler/app-data
git clone remote one-vbranch-on-integration
(cd one-vbranch-on-integration
  $CLI project add --switch-to-integration "$(git rev-parse --symbolic-full-name @{u})"
  $CLI branch create virtual
)

git clone remote one-vbranch-on-integration-one-commit
(cd one-vbranch-on-integration-one-commit
  $CLI project add --switch-to-integration "$(git rev-parse --symbolic-full-name @{u})"
  $CLI branch create virtual
  echo change >> file
  echo in-index > new && git add new
  $CLI branch commit virtual -m "virtual branch change in index and worktree"
)

