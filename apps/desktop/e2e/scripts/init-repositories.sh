#!/usr/bin/env bash

set -eu -o pipefail

CLI=${1:?The first argument is the GitButler CLI}

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

git init remote
(
  cd remote
  echo first >file
  git add . && git commit -m "init"
)

git clone remote one-vbranch-on-integration
(
  cd one-vbranch-on-integration
  $CLI project add --switch-to-integration "$(git rev-parse --symbolic-full-name "@{u}")"
  $CLI branch create virtual
)
