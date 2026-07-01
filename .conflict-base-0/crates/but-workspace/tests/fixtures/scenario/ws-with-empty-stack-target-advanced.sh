#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit, with two stacks inside, one empty and one with a commit.
# Additionally, the target (origin/main) has advanced past the workspace base, while the
# `gitbutler/target` anchor still points at the base. This makes the merge-base segment carry the
# `gitbutler/target` reference node above the base commit in the editor graph. See
# `ws-with-empty-stack.sh` for the variant where the target is caught up.
git init
commit M
setup_target_to_match_main
git branch gitbutler/target main

git branch B
git checkout -b A
  commit A
git checkout B
create_workspace_commit_once A B

# Advance the target past the workspace base while leaving `gitbutler/target` at the base.
git checkout main
commit-file X.txt X
git update-ref refs/remotes/origin/main main
git checkout gitbutler/workspace
