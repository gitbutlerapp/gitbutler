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
tick

git init remote
(cd remote
  echo first > file
  git add . && git commit -m "init"
)

export GITBUTLER_CLI_DATA_DIR=../user/gitbutler/app-data
git clone remote multiple-commits 
(cd multiple-commits
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"

  $CLI branch create --set-default first_branch
  echo asdf >> foo
  tick
  $CLI branch commit first_branch -m "some commit"

  $CLI branch create --set-default virtual
  echo change >> file
  tick
  $CLI branch commit virtual -m "first commit"
  echo change2 >> file
  tick
  $CLI branch commit virtual -m "second commit"
  echo change3 >> file
  tick
  $CLI branch commit virtual -m "third commit"
)
