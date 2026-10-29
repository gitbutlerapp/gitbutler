#!/bin/bash

set -euo pipefail

# Like project-in-single-branch-with-worktree.sh, but `worktree-W` is detached at its
# commit and branch W is gone, leaving the worktree's segment anonymous.
bash "$(dirname "$0")/project-in-single-branch-with-worktree.sh"

git -C worktree-W checkout --detach
git -C local-clone branch -D W
