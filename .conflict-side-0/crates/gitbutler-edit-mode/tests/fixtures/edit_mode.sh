#!/usr/bin/env bash
set -eu -o pipefail
CLI=${1:?The first argument is the GitButler CLI}

export GITBUTLER_CLI_DATA_DIR=../user/gitbutler/app-data

git init repo
(cd repo
  git config user.name "Author"
  git config user.email "author@example.com"
  echo a > file
  git add . && git commit -m "init"
)

# Setup:
# * 6a0c4bd (HEAD -> gitbutler/workspace) GitButler Workspace Commit
# * 95d4a63 foobar
# | * 1e2a3a8 (right) right
# |/
# | * f3d2634 (left) left
# |/
# * 7950f06 (origin/main, origin/HEAD, main) init
# Where "left" and "right" contain changes which conflict with each other
git clone repo conficted_entries_get_written_when_leaving_edit_mode
(cd conficted_entries_get_written_when_leaving_edit_mode
  git config user.name "Author"
  git config user.email "author@example.com"
  git checkout -b left
  echo left > conflict
  git add . && git commit -m "left"
  git checkout main
  git checkout -b right
  echo right > conflict
  git add . && git commit -m "right"
  git checkout main
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name origin/main)"
  echo b > file
  $CLI branches create --set-default branchy
  $CLI branches commit  branchy --message foobar
)