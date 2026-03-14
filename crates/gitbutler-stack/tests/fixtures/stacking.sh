#!/usr/bin/env bash
set -eu -o pipefail
git init --initial-branch=main remote
(cd remote
  git config user.name "Author"
  git config user.email "author@example.com"
  echo first > file
  git add . && git commit -m "init"
)

git clone remote multiple-commits 
(cd multiple-commits
  git config user.name "Author"
  git config user.email "author@example.com"
  git branch existing-branch

  git checkout -b first_branch
  echo asdf >> foo
  git add foo && git commit -m "some commit"

  git checkout main
  git checkout -b virtual
  echo change >> file
  git add file && git commit -m "first commit"
  echo change2 >> file
  git add file && git commit -m "second commit"
  echo change3 >> file
  git add file && git commit -m "third commit"

  git checkout -b gitbutler/workspace
  git merge --no-ff -m "GitButler Workspace Commit" first_branch
)
