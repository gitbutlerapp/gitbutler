#!/bin/bash

set -euo pipefail

# The stacks project in workspace mode with branch1 applied, and the linked worktree
# `worktree-W` on W, stacked on W-base, resting on the target.
bash "$(dirname "$0")/project-with-stacks.sh" "$@"

pushd local-clone
"$BUT" apply branch1
# Worktrees on disk when adoption first runs are adopted as archived, so it has already run.
python3 - .git/gitbutler/but.sqlite <<'PYTHON'
import sqlite3
import sys

with sqlite3.connect(sys.argv[1]) as database:
    database.execute("INSERT OR IGNORE INTO worktree_adoption (id) VALUES (1)")
PYTHON
git worktree add -b W-base ../worktree-W origin/master
pushd ../worktree-W
echo "W-base" > w_base_file
git add w_base_file
git commit -m "W-base: first commit"
git checkout -b W
echo "W" > w_file
git add w_file
git commit -m "W: first commit"
popd
popd
