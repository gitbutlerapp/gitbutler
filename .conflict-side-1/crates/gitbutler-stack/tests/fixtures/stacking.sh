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

  $CLI branch create --set-default first_branch
  echo asdf >> foo
  $CLI branch commit first_branch -m "some commit"

  $CLI branch create --set-default virtual
  echo change >> file
  $CLI branch commit virtual -m "first commit"
  echo change2 >> file
  $CLI branch commit virtual -m "second commit"
  echo change3 >> file
  $CLI branch commit virtual -m "third commit"
)
