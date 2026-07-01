#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
echo "a stack with two segments, one commit each" >.git/description

commit M1
setup_target_to_match_main
git checkout -b A1
  commit-file file A1
git checkout -b A2
  commit-file file A2
git checkout -b unrelated main
  commit-file unrelated U1
git checkout main
