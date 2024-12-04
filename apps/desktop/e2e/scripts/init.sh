#!/usr/bin/env bash

set -eu -o pipefail

TEMP_DIR=${1:?The first argument is a temp dir}
# Convert to absolute path
TEMP_DIR=$(realpath "$TEMP_DIR")


CLI=$(realpath "../../target/debug/gitbutler-cli")

DATA_DIR="$HOME/.local/share/com.gitbutler.app.test"
if [ -d "$DATA_DIR" ]; then
  rm -rf $DATA_DIR
fi

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

if [ -d "$TEMP_DIR" ]; then
  rm -rf "$TEMP_DIR"
fi
mkdir "$TEMP_DIR"

(
  cd "$TEMP_DIR"
  git init remote
  cd remote
  setGitDefaults
  echo first >file
  git add . && git commit -m "init"
)

