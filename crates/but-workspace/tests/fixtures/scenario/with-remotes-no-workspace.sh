#!/usr/bin/env bash

### General Description

# Various directories with different scenarios for testing stack information without a workspace commit available.
set -eu -o pipefail

function set_author() {
  local author=${1:?Author}

  unset GIT_AUTHOR_NAME
  unset GIT_AUTHOR_EMAIL

  git config user.name $author
  git config user.email $author@example.com
}

git init remote
(cd remote
  touch file
  git add . && git commit -m init-integration

  git checkout -b A
  touch file-in-A && git add . && git commit -m "new file in A"
  echo change >file-in-A && git commit -am "change in A"

  git checkout main
)

# The remote has a new commit, but is fast-forwardable.
git clone remote remote-tracking-advanced-ff
(cd remote-tracking-advanced-ff
  git checkout -b A origin/A
  git reset --hard @~1
)

# The remote has a new commit, and local has a new commit in an unreconcilable way.
cp -R remote-tracking-advanced-ff remote-diverged
(cd remote-diverged
  set_author local-user
  echo other-change >file-in-A && git commit -am "local change in A"
)

git clone remote nothing-to-push
(cd nothing-to-push
  git checkout -b A origin/A
)
