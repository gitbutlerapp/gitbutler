#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# Two stacks over one shared parent branch: A carries a commit, B-on-A and C-on-A each
# add one on top of it, and the ws commit merges both tips. The shared A segment
# legitimately displays under BOTH stacks.
git init
commit M

setup_target_to_match_main

git checkout -b A
  commit A1

git checkout -b B-on-A
  commit B1

git checkout -b C-on-A A
  commit C1

create_workspace_commit_once B-on-A C-on-A
