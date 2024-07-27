#!/usr/bin/env bash
set -eu -o pipefail

git init remote
(cd remote
  echo first > file
  git add . && git commit -m "init"
)

git clone remote single-branch-no-vbranch

git clone remote single-branch-no-vbranch-multi-remote
(cd single-branch-no-vbranch-multi-remote
  git remote add other-origin ../remote
  git fetch other-origin
)


