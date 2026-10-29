#!/usr/bin/env bash
set -eu -o pipefail

git init -b master local
(cd local
  git commit --allow-empty -m "Initial commit"
)

git init --bare remote
(cd local
  git remote add origin ../remote
  git push origin master
  git fetch origin '+refs/heads/*:refs/remotes/origin/*'
)
