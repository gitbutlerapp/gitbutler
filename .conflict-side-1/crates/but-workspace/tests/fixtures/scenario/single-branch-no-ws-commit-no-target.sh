#!/usr/bin/env bash

### Description
source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
echo "Single commit, no main remote/target, no ws commit, but ws-reference" >.git/description

commit M1
git checkout -b gitbutler/workspace

for branch in "$@"; do
  git branch "$branch"
done