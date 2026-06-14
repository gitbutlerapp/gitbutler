#!/usr/bin/env bash

### Description
# Workspace commits whose octopus lists the base as a *repeated* direct parent — the
# representation for multiple fully-empty stacks (each empty stack's head IS the base, so it
# appears once per empty stack). `git merge` cannot produce this (merging an ancestor is a
# no-op), so the workspace commit is built with `git commit-tree` and explicit parents.
set -eu -o pipefail
source "${BASH_SOURCE[0]%/*}/shared.sh"

# One real stack `S` plus two fully-empty stacks (`empty-A`, `empty-B`) resting on the base.
# Octopus parents: [S-tip, base, base] — base twice, once per empty stack.
git init two-empty-at-base
(cd two-empty-at-base
  commit init
    setup_target_to_match_main
    git branch empty-A
    git branch empty-B
  git checkout -b S
    commit s1
  tree=$(git rev-parse "S^{tree}")
  ws=$(git commit-tree "$tree" -p S -p empty-A -p empty-B -m "GitButler Workspace Commit")
  git checkout -b gitbutler/workspace "$ws"
)
