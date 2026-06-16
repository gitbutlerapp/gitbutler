#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A single stack of two branches, A on top of B, with no workspace branch,
# workspace commit, or virtual-branch metadata. HEAD is checked out directly on
# the top of the stack (A). The bottom branch B is historically integrated into
# the target, and the target ref (origin/main) has advanced one commit (X) past
# B, while A remains to be integrated.
git init
commit M1
setup_target_to_match_main

git checkout -b B
commit-file B.txt B1

git checkout -b A
commit-file A.txt A1

# Advance the target one commit beyond the integrated bottom branch B.
git checkout -b upstream-main B
commit-file X.txt X1
git update-ref refs/remotes/origin/main upstream-main

git checkout A
git branch -D upstream-main
