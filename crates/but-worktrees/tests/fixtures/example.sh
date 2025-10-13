#!/usr/bin/env bash
set -eu -o pipefail
CLI=${1:?The first argument is the GitButler CLI}

git init remote
(cd remote
  git config user.name "Author"
  git config user.email "author@example.com"

  echo "initial content" > file.txt
  git add . && git commit -m "init"
)

CHANGE_ID=0
function commit_stack() {
  local stack="${1:?}"
  local message="${2:?}"
  ((CHANGE_ID += 1))
  CHANGE_ID=$CHANGE_ID $CLI branch commit "$stack" -m "$message"
}

export GITBUTLER_CLI_DATA_DIR=../user/gitbutler/app-data

# Example scenario: A basic workspace with a single stack
git clone remote example-scenario
(cd example-scenario
  git config user.name "Author"
  git config user.email "author@example.com"

  git branch existing-branch
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"

  # Create example stack
  $CLI branch create --set-default example-stack
  echo "line 1" >> file.txt
  commit_stack "example-stack" "Add line 1"
  echo "line 2" >> file.txt
  commit_stack "example-stack" "Add line 2"

  # Switch back to workspace
  git checkout gitbutler/workspace
)
