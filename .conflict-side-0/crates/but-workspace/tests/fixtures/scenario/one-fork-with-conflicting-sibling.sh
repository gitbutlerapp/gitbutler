#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# Like one-fork, but with an additional local branch that conflicts with A by
# adding the same path with different content.
git init
commit-file init
setup_target_to_match_main

git branch B
git checkout -b A
  commit-file A
git checkout B
  commit-file B
git checkout main
  commit M
git checkout -b add-A-too main
  echo "different A" >A
  git add A
  git commit -m "add a different A"
git checkout main

setup_remote_tracking B B "move"
echo 'ref: refs/remotes/origin/main' > .git/refs/remotes/origin/HEAD
