#!/bin/bash

set -eu -o pipefail

git init

echo "base" >base && git add . && git commit -m "base"
echo "m1" >m1 && git add . && git commit -m "m1"

# A linked worktree 'wt-a' on 'feat-a', forked before the target tip,
# with one extra commit and an uncommitted change.
git worktree add -b feat-a wt-a HEAD~1
(cd wt-a
  echo "a1" >a1 && git add a1 && git commit -m "a1"
  echo "dirty" >>a1
)

# A linked worktree 'wt-b' for tests to archive.
git worktree add -b feat-b wt-b HEAD~1

# A linked worktree 'wt-gone' whose checkout was removed from disk (prunable).
git worktree add -b feat-gone wt-gone HEAD~1
rm -rf wt-gone
