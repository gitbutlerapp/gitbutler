#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### A stack of two branches where the *lower* branch was SQUASH-merged upstream,
### and the *upper* branch carries only a no-change commit of its own.
###
### Stack layout (top to bottom): top -> bottom -> base
### - bottom: a real commit that carries a gitbutler change-id, pushed, and then
###   squash-merged into the target (origin/main) as a NEW commit with the SAME
###   change-id but a DIFFERENT sha (same content/tree).
### - top: a NON-empty branch stacked on bottom whose single commit introduces no
###   changes of its own (an `--allow-empty` commit). It was never separately merged.
###
### `top` contributed nothing that was merged and is a distinct named branch the user
### never merged, so it must survive `but pull` even though the genuinely squash-merged
### `bottom` below it is removed.

git-init-frozen

commit-file base

git checkout -b bottom
commit-file bottom
# Give bottom a real change-id so the upstream squash can be matched by change-id.
new_bottom=$(add_change_id_to_given_commit 1 refs/heads/bottom)
git update-ref refs/heads/bottom "$new_bottom"

# top is a NON-empty branch stacked on bottom whose only commit introduces no changes.
git checkout -b top
git commit --allow-empty -m "feat: target branch tip (no changes)"

# Both branches were pushed.
mkdir -p .git/refs/remotes/origin
cp .git/refs/heads/bottom .git/refs/remotes/origin/bottom
cp .git/refs/heads/top .git/refs/remotes/origin/top

# bottom's PR was SQUASH-merged upstream: main advances with a NEW commit that
# reproduces bottom's tree, carrying the SAME change-id but a different sha.
git checkout main
bottom_tree=$(git rev-parse refs/heads/bottom^{tree})
squashed=$(git commit-tree "$bottom_tree" -p main -m "squash bottom")
git update-ref refs/heads/main "$squashed"
# Re-stamp main's tip with bottom's change-id (same id, different sha than bottom).
new_main=$(add_change_id_to_given_commit 1 refs/heads/main)
git update-ref refs/heads/main "$new_main"

setup_target_to_match_main

git checkout top
create_workspace_commit_once top
