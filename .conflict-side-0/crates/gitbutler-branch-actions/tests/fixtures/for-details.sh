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
git clone remote complex-repo
(cd complex-repo
  for round in $(seq 5); do
    echo main >> file
    git commit -am "main-$round"
  done

  local_tracking_ref="$(git rev-parse --symbolic-full-name @{u})";

  git checkout -b feature main
  for round in $(seq 100); do
    echo feature >> file
    git commit -am "feat-$round"
  done

  git checkout main
  $CLI project add --switch-to-workspace "$local_tracking_ref"
  for round in $(seq 10); do
    echo virtual-main >> file
    # We will now use a canned branch name here
    $CLI branch commit --message "virt-$round" a-branch-1
  done

  git checkout -b non-virtual-feature main
  for round in $(seq 50); do
    echo non-virtual-feature >> file
    git commit -am "non-virtual-feat-$round"
  done

  # pretend the remote is at the same state as our local `main`
  # This previously wanted to update the remote to match the local main, we no
  # longer have a "main" virutal branch, so this has been changed to the canned
  # branch.
  git update-ref refs/remotes/origin/main a-branch-1
)
