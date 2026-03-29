#!/usr/bin/env bash
set -eu -o pipefail

git init -b master local
(cd local
  git config user.name gitbutler-test
  git config user.email gitbutler-test@example.com
  git config commit.gpgsign false
  git commit --allow-empty -m "Initial commit"
)

git init --bare remote
(cd local
  git remote add origin ../remote
  git push origin master
)
