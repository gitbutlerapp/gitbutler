#!/bin/bash

set -euo pipefail

# The three-branch stack master <- A <- B <- C, with the linked worktree `worktree-W`
# on branch W resting on B.
bash "$(dirname "$0")/project-in-single-branch-three-branch-stack.sh"

python3 - "$E2E_TEST_APP_DATA_DIR/gitbutler/settings.json" <<'PYTHON'
import json
import os
import sys

path = sys.argv[1]
settings = json.load(open(path)) if os.path.exists(path) else {}
settings.setdefault("featureFlags", {})["worktreeManipulation"] = True
os.makedirs(os.path.dirname(path), exist_ok=True)
with open(path, "w") as file:
    json.dump(settings, file)
PYTHON

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
