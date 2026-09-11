#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

# Local A adds `x`, removes it, and adds it again; origin/A holds a rewritten
# copy of the first `add x` plus one commit of its own. Only the first local
# `add x` is already upstream: the tip must still end with `x` present.
git-init-frozen
commit-file M
setup_target_to_match_main

commit-file x
git rm -q x && git commit -q -m "remove x"
commit-file x
git branch A
git reset --hard @~3

tick_committer
commit-file x
commit-file only-on-remote
git update-ref refs/remotes/origin/A HEAD
git reset --hard @~2

create_workspace_commit_once main A
