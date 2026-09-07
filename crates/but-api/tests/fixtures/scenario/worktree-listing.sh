#!/usr/bin/env bash

set -eu -o pipefail

git init
echo "linked worktrees under worktrees/, each on a branch of its own name, with reflog entries stamped so recency is observable" >.git/description

git config user.name GitButler
git config user.email gitbutler@example.com

echo one >file.txt
git add file.txt && git commit -m "one"
echo two >file.txt
git commit -am "two"

# Two worktrees share a day so ordering falls back to the name, and one has no reflog at all.
GIT_COMMITTER_DATE="2000-01-03 00:00:00 +0000" git worktree add -b older worktrees/older HEAD~1
GIT_COMMITTER_DATE="2000-01-04 00:00:00 +0000" git worktree add -b newer worktrees/newer HEAD~1
GIT_COMMITTER_DATE="2000-01-02 00:00:00 +0000" git worktree add -b same-day-b worktrees/same-day-b HEAD~1
GIT_COMMITTER_DATE="2000-01-02 00:00:00 +0000" git worktree add -b same-day-a worktrees/same-day-a HEAD~1
git -c core.logAllRefUpdates=false worktree add -b nolog worktrees/nolog HEAD~1

# 'older' has uncommitted work, which `git worktree remove` refuses without force.
echo dirty >worktrees/older/file.txt
