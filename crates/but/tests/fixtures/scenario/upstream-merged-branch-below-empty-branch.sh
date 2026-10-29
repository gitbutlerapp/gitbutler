#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### A stack of two branches where only the *lower* branch merged upstream.
###
### Stack layout (top to bottom): top -> bottom -> base
### - bottom: has a real commit, pushed, and merged into the target (origin/main).
### - top: empty branch stacked on bottom, pushed. Its remote ref points at bottom's
###   (now-merged) commit, which is an ancestor of the new target.
###
### `top` contributed no commits of its own and was never merged, so it must survive
### `but pull` even though `bottom` below it is removed.

git-init-frozen

commit-file base

git checkout -b bottom
commit-file bottom

# top is stacked on bottom and has no commits of its own.
git checkout -b top

# Both branches were pushed; top is empty so it sits at bottom's tip.
mkdir -p .git/refs/remotes/origin
cp .git/refs/heads/bottom .git/refs/remotes/origin/bottom
cp .git/refs/heads/top .git/refs/remotes/origin/top

# bottom's PR merged upstream: main advances to include bottom via a merge commit.
git checkout main
git merge --no-ff -m "merge bottom" bottom

setup_target_to_match_main

git checkout top
create_workspace_commit_once top
