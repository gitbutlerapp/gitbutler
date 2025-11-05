#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $GITBUTLER_CLI_DATA_DIR"
echo "BUT_TESTING $BUT_TESTING"
echo "COMMIT MESSAGE: $1"
echo "FILE PATH: $2"
echo "FILE CONTENT: $3"

# Create a new branch in the remote project, add a file, and commit it.
pushd remote-project
  git checkout master
  echo "$3" >> "$2"
  git add "$1"
  git commit -m "$1"
popd
