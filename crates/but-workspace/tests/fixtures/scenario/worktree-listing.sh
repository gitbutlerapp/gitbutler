#!/bin/bash

set -eu -o pipefail

git init

echo "base" >base && git add . && git commit -m "base"
echo "m1" >m1 && git add . && git commit -m "m1"

# Every reflog entry below carries a distinct committer day so recency is observable.

# A linked worktree 'wt-a' on 'feat-a', checked out on day 3, with a commit of its own on day 5
# and an uncommitted change.
GIT_COMMITTER_DATE="2000-01-03 00:00:00 +0000" git worktree add -b feat-a wt-a HEAD~1
(cd wt-a
  echo "a1" >a1 && git add a1 && GIT_COMMITTER_DATE="2000-01-05 00:00:00 +0000" git commit -m "a1"
  echo "dirty" >>a1
)

# A clean linked worktree 'wt-b' on 'feat-b', checked out on day 4, whose branch was then moved
# from the main checkout on day 6 - only the branch log sees that.
GIT_COMMITTER_DATE="2000-01-04 00:00:00 +0000" git worktree add -b feat-b wt-b HEAD~1
GIT_COMMITTER_DATE="2000-01-06 00:00:00 +0000" git update-ref refs/heads/feat-b HEAD

# A linked worktree 'wt-detached' on a detached HEAD at the base commit, with only a HEAD log.
git worktree add --detach wt-detached HEAD~1

# A linked worktree 'wt-nolog' on 'feat-nolog' created without any reflog.
git -c core.logAllRefUpdates=false worktree add -b feat-nolog wt-nolog HEAD~1

# A linked worktree 'wt-gone' whose checkout was removed from disk (prunable).
git worktree add -b feat-gone wt-gone HEAD~1
rm -rf wt-gone
