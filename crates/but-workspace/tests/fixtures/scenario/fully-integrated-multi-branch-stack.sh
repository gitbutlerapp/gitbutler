#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# Two stacks are merged into a workspace commit:
# - A stack with A on top of C, where both A and C are historically integrated.
# - A separate B stack.
git init
commit M1
setup_target_to_match_main

git checkout -b C
commit-file C.txt C1

git checkout -b A
commit-file A.txt A1

git checkout main
git checkout -b B
commit-file B.txt B1

create_workspace_commit_once A B

git update-ref refs/remotes/origin/main A
