#!/usr/bin/env bash
set -eu -o pipefail
CLI=${1:?The first argument is the GitButler CLI}


git init remote
(cd remote
  echo first > file
  git add . && git commit -m "init"
)

export GITBUTLER_CLI_DATA_DIR=../user/gitbutler/app-data
git clone remote multiple-commits
(cd multiple-commits
  git config user.name "Author"
  git config user.email "author@example.com"

  git branch existing-branch
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"

  $CLI branch create --set-default other_stack
  echo change0 >> other_file
  $CLI branch commit other_stack -m "commit 0"

  $CLI branch create --set-default my_stack
  echo change1 >> file
  $CLI branch commit my_stack -m "commit 1"
  echo change2 >> file
  $CLI branch commit my_stack -m "commit 2"
  echo change3 >> file
  $CLI branch commit my_stack -m "commit 3"

  $CLI branch series my_stack -s "top-series"
  echo change4 >> file
  $CLI branch commit my_stack -m "commit 4"
  echo change5 >> file
  $CLI branch commit my_stack -m "commit 5"
  echo change6 >> file
  $CLI branch commit my_stack -m "commit 6"
)
