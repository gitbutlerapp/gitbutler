#!/bin/bash

set -eu -o pipefail

git init
git commit --allow-empty -m M

# A bare clone with a linked worktree of its own: their git dirs contain no
# `.git` path component for kind heuristics to latch onto.
git clone --bare . bare.git
git -C bare.git worktree add -b feat-bare ../wt-bare
