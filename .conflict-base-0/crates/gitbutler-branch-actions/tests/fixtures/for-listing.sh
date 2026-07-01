#!/usr/bin/env bash
set -eu -o pipefail

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

git init --initial-branch=main remote
(cd remote
  git config user.name "Author"
  git config user.email "author@example.com"
  echo first > file
  git add . && git commit -m "init"
)

git clone remote one-vbranch-in-workspace
(cd one-vbranch-in-workspace
  git config user.name "Author"
  git config user.email "author@example.com"
  git branch virtual
  git checkout -b gitbutler/workspace virtual
  git commit --allow-empty -m "GitButler Workspace Commit"
)

git clone remote one-vbranch-in-workspace-one-commit
(cd one-vbranch-in-workspace-one-commit
  git config user.name "Author"
  git config user.email "author@example.com"
  git checkout -b virtual
  echo change >> file
  echo in-index > new && git add new
  tick
  git add file && git commit -m "virtual branch change in index and worktree"
  git checkout -b gitbutler/workspace
  git commit --allow-empty -m "GitButler Workspace Commit"
)

git clone remote one-branch-one-commit-other-branch-without-commit
(cd one-branch-one-commit-other-branch-without-commit
  git config user.name "Author"
  git config user.email "author@example.com"

  git checkout -b feature main
  echo change >> file
  git add . && git commit -m "change standard git feature branch"

  git checkout -b other-feature main
)
