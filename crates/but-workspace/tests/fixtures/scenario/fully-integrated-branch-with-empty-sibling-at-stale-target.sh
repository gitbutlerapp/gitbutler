#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# Like fully-integrated-branch-with-stale-target-parent, but the second lane is
# a named empty branch B at the old target commit M instead of a bare edge to it.
# Stack A is fully integrated (the target ref advanced to contain it), while B
# must survive the integration: it rebases onto the advanced target and the
# workspace commit keeps following it.
git init
commit-file M.txt M
setup_target_to_match_main

git branch B

git checkout -b A
commit-file A.txt A
git update-ref refs/remotes/origin/main A

# `git merge` refuses to create a merge commit with an ancestor, so build the
# two-parent workspace commit (stack tip + empty branch at the old target)
# directly.
tick
git checkout -b gitbutler/workspace
git update-ref refs/heads/gitbutler/workspace \
  "$(git commit-tree "A^{tree}" -p A -p B -m "GitButler Workspace Commit")"
