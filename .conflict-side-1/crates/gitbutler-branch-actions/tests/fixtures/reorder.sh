#!/usr/bin/env bash
set -eu -o pipefail
CLI=${1:?The first argument is the GitButler CLI}

function tick () {
  if test -z "${tick+set}"; then
    tick=1675176957
  else
    tick=$(($tick + 60))
  fi
  GIT_COMMITTER_DATE="$tick +0100"
  GIT_AUTHOR_DATE="$tick +0100"
  export GIT_COMMITTER_DATE GIT_AUTHOR_DATE
}

git init remote
(cd remote
  echo a > file
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

git clone remote multiple-commits-small
(cd multiple-commits-small
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

  $CLI branch series my_stack -s "top-series"
  echo change4 >> file
  $CLI branch commit my_stack -m "commit 2"
)

git clone remote multiple-commits-empty-top
(cd multiple-commits-empty-top
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

  $CLI branch series my_stack -s "top-series"
)

git clone remote overlapping-commits
tick
(cd overlapping-commits
  git config user.name "Author"
  git config user.email "author@example.com"
  
  git branch existing-branch
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"

  $CLI branch create --set-default other_stack
  echo change0 >> other_file
  $CLI branch commit other_stack -m "commit 0"

  $CLI branch create --set-default my_stack
  echo x > file
  $CLI branch commit my_stack -m "commit 1"
  tick
  echo y > file
  $CLI branch commit my_stack -m "commit 2"

  $CLI branch series my_stack -s "top-series"
)
