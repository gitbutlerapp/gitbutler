#!/usr/bin/env bash
set -eu -o pipefail

git init --initial-branch=main remote
(cd remote
  git config user.name "Author"
  git config user.email "author@example.com"
  echo first > file
  git add . && git commit -m "init"
)

git clone remote workspace-migration
(cd workspace-migration
  git config user.name "Author"
  git config user.email "author@example.com"
  git branch virtual
  git checkout -b gitbutler/workspace virtual
  git commit --allow-empty -m "GitButler Workspace Commit"
  git checkout -b gitbutler/integration
)
