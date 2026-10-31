#!/bin/bash

set -euo pipefail

# The three-branch stack master <- A <- B <- C, with the linked worktree `worktree-W`
# on branch W resting on B.
bash "$(dirname "$0")/project-in-single-branch-three-branch-stack.sh"

pushd local-clone
# Worktrees on disk when adoption first runs are adopted as archived, so it has already run.
python3 - .git/gitbutler/but.sqlite <<'PYTHON'
import sqlite3
import sys

with sqlite3.connect(sys.argv[1]) as database:
    database.execute("INSERT OR IGNORE INTO worktree_adoption (id) VALUES (1)")
PYTHON
git worktree add -b W ../worktree-W B
pushd ../worktree-W
echo "W" > W.txt
git add W.txt
git commit -m "W: first commit"
popd
popd
