#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A pushed main branch with two forked-off feature branches with the same base. One of these branches is a remote tracking branch.
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

setup_remote_tracking B B "move"
