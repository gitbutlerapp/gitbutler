#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
echo "two stacks with different bases, upstream modified file, uncommitted changes" >.git/description

# Initial base with a file
echo "original" >file.txt
git add file.txt
commit "initial"

# Advance main: upstream modifies file.txt
echo "upstream-changed" >file.txt
git add file.txt
commit "upstream changes file"
setup_target_to_match_main

# Stack A: fork from main~1 (old base, before upstream changed file.txt)
git checkout -b A main~1
echo "A-content" >a-only.txt
git add a-only.txt
commit "A adds a-only"

# Stack B: fork from main (new base, includes upstream's change to file.txt)
git checkout -b B main
echo "B-content" >b-only.txt
git add b-only.txt
commit "B adds b-only"

# Create workspace merge commit from both stacks
# B is current (has upstream-changed file.txt), merge A in
# The merge base between A and B is main~1 where file.txt="original"
# A has file.txt="original" (unchanged from its base), B has file.txt="upstream-changed"
# So the merge picks B's (upstream) version of file.txt for the workspace tree.
create_workspace_commit_once B A

# Uncommitted worktree change: modify file.txt.
# This change is relative to the workspace merge tree (which has "upstream-changed").
# When committing to stack A, the cherry-pick does:
#   base = HEAD^{tree} (workspace: file.txt="upstream-changed")
#   ours = A-tip^{tree} (A: file.txt="original")
#   theirs = HEAD^{tree} + changes (file.txt="worktree-modified")
# The cherry-pick sees (base→ours) changed "upstream-changed" to "original"
# and (base→theirs) changed "upstream-changed" to "worktree-modified" → CONFLICT
echo "worktree-modified" >file.txt
