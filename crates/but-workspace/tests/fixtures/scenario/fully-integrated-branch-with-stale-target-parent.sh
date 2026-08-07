#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A stack A sits on the old target commit M, and the workspace commit merges
# both A and the old target directly (an unnamed empty lane at the base). The
# target ref has advanced to contain A, exactly like right after `but land`
# fast-forwarded it. Integrating must reparent the workspace commit onto the
# advanced target instead of leaving it on the stale base.
git init
commit-file M.txt M
setup_target_to_match_main

git checkout -b A
commit-file A.txt A
git update-ref refs/remotes/origin/main A

# `git merge` refuses to create a merge commit with an ancestor, so build the
# two-parent workspace commit (stack tip + stale target) directly.
tick
git checkout -b gitbutler/workspace
git update-ref refs/heads/gitbutler/workspace \
  "$(git commit-tree "A^{tree}" -p A -p main -m "GitButler Workspace Commit")"
