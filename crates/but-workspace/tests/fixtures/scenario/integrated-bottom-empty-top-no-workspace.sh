#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A single stack in single-branch mode where the checked-out top branch A is
# empty and points at the same commit as the bottom branch B. The bottom branch B
# is historically integrated into the target, and origin/main has advanced one
# commit past B.
git init
commit M1
setup_target_to_match_main

git checkout -b B
commit-file B.txt B1
git branch A

git checkout -b upstream-main B
commit-file X.txt X1
git update-ref refs/remotes/origin/main upstream-main

git checkout A
git branch -D upstream-main
