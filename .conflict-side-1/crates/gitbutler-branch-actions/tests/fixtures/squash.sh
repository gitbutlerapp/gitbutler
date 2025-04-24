#!/usr/bin/env bash
set -eu -o pipefail
CLI=${1:?The first argument is the GitButler CLI}


git init remote
(cd remote
  echo a > file
  git add . && git commit -m "init"
)

export GITBUTLER_CLI_DATA_DIR=../user/gitbutler/app-data

# Scenario:
# - commit 5 (a-branch-3)
# - commit 4 (a-branch-2)
# - commit 3
# - commit 2
# - commit 1 (a-branch-1)
git clone remote multiple-commits
(cd multiple-commits
  git config user.name "Author"
  git config user.email "author@example.com"

  git branch existing-branch
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"

  # Create the stack
  $CLI branch create --set-default my_stack

  # Change 1 is in the default stacked branch
  echo change1 >> file1
  $CLI branch commit my_stack -m "commit 1"

  # Create a new stacked branch
  $CLI branch series my_stack -s "a-branch-2"

  # Changes 2 and 3 are in the same file, with 2 overwriting 3
  echo change2 > file2_3
  $CLI branch commit my_stack -m "commit 2"
  echo change3 > file2_3
  $CLI branch commit my_stack -m "commit 3"

  # Commit 4 is in a different file
  echo change4 > file4
  $CLI branch commit my_stack -m "commit 4"

  # Create a new stacked branch
  $CLI branch series my_stack -s "a-branch-3"

  # Commit 5 is in a different file
  echo change5 >> file5
  $CLI branch commit my_stack -m "commit 5"
)
