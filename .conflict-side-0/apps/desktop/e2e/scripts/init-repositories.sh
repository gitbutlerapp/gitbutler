#!/usr/bin/env bash

set -eu -o pipefail

TEMP_DIR="/tmp/gb-e2e-repos"

CLI=${1:?The first argument is the GitButler CLI}
# Convert to absolute path
CLI=$(realpath "$CLI")

function setGitDefaults() {
  git config user.email "test@example.com"
  git config user.name "Test User"
  git config init.defaultBranch master
}

function tick() {
  if test -z "${tick+set}"; then
    tick=1675176957
  else
    tick=$($tick + 60)
  fi
  GIT_COMMITTER_DATE="$tick +0100"
  GIT_AUTHOR_DATE="$tick +0100"
  export GIT_COMMITTER_DATE GIT_AUTHOR_DATE
}
tick

if [ ! -d "$TEMP_DIR" ]; then
  mkdir "$TEMP_DIR"
fi
cd "$TEMP_DIR"

git init remote

(
  cd remote

  setGitDefaults

  echo first >file
  git add . && git commit -m "init"
)

git clone remote one-vbranch-on-integration

# This code will be useful for scenarios that assumes a project
# already exists.
# (
#   cd one-vbranch-on-integration
#   $CLI project add --switch-to-integration "$(git rev-parse --symbolic-full-name "@{u}")"
#   $CLI branch create virtual
# )
