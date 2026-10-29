#!/usr/bin/env bash
set -eu -o pipefail

git init --initial-branch=main remote
(cd remote
  git config user.name "Author"
  git config user.email "author@example.com"
  echo first > file
  git add . && git commit -m "init"
)

git clone remote complex-repo
(cd complex-repo
  git config user.name "Author"
  git config user.email "author@example.com"
  for round in $(seq 5); do
    echo main >> file
    git commit -am "main-$round"
  done

  git checkout -b feature main
  for round in $(seq 10); do
    echo feature >> file
    git commit -am "feat-$round"
  done

  git checkout main
  git checkout -b a-branch-1
  for round in $(seq 3); do
    echo virtual-main >> file
    git commit -am "virt-$round"
  done
  git checkout -b gitbutler/workspace
  git commit --allow-empty -m "GitButler Workspace Commit"

  git checkout -b non-virtual-feature main
  for round in $(seq 10); do
    echo non-virtual-feature >> file
    git commit -am "non-virtual-feat-$round"
  done

  git update-ref refs/remotes/origin/main a-branch-1
)
