#!/usr/bin/env bash

### General Description

# Disjoint scenarios related to remote branches that are merged in.
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init 01-one-rewritten-one-local-after-push
(cd 01-one-rewritten-one-local-after-push
  echo "A setup that demands for a force-push

We change the name of the first commit and also need the similarity to be detected by changeset" >.git/description
  commit init && setup_target_to_match_main
  git checkout -b A
    git branch soon-A-remote
    echo A-new >file && git add file && git commit -m A1
    echo A-new >other && git add other && git commit -m A2
  git checkout soon-A-remote
    echo A-new >file && git add file && git commit -m "A1 (same but different)"
    setup_remote_tracking soon-A-remote A "move"

  git checkout A
  create_workspace_commit_once A
)

cp -R 01-one-rewritten-one-local-after-push 01.5-one-rewritten-one-local-after-push-merge
(cd 01.5-one-rewritten-one-local-after-push-merge
  echo "On the remote, a rewritten/rebased commit we have locally is merged back into target." >.git/description
  git checkout -b soon-main-remote origin/main
    git merge --no-ff origin/A -m "merge origin/A"
    setup_remote_tracking soon-main-remote main "move"

  git checkout gitbutler/workspace
)

git init 02-diverged-remote
(cd 02-diverged-remote
  echo "A setup that demands for a force-push

The tip of the local branch isn't in the ancestry of the remote anymore." >.git/description
  commit init && setup_target_to_match_main
  git checkout -b A
    commit A1
    git branch soon-A-remote
    commit A2
  git checkout soon-A-remote
    commit A3
    setup_remote_tracking soon-A-remote A "move"

  git checkout A
  create_workspace_commit_once A
)

cp -R 02-diverged-remote 02.5-diverged-remote-merge
(cd 02.5-diverged-remote-merge
  echo "A remote sharing a commit with a stack and its own commit gets merged.

We'd not want to see the remote unique commit anymore as it's also considered integrated." >.git/description
  git checkout main
    git merge --no-ff origin/A
    git rev-parse @ >.git/refs/remotes/origin/main

  git checkout gitbutler/workspace
)

git init 03-remote-one-behind
(cd 03-remote-one-behind
  echo "A can be pushed as it has local, unpushed commits" >.git/description
  commit init && setup_target_to_match_main
  git checkout -b A
    commit A1
    git rev-parse @ >.git/refs/remotes/origin/A
    commit A2

  create_workspace_commit_once A
)

cp -R 03-remote-one-behind 03.5-remote-one-behind-merge-no-ff
(cd 03.5-remote-one-behind-merge-no-ff
  echo "Remote origin/A is merged back (with forceful merge commit) while there are still local commits." >.git/description
  git checkout main
    git merge --no-ff origin/A
    git rev-parse @ >.git/refs/remotes/origin/main

  git checkout gitbutler/workspace
)

git init 04-remote-one-ahead-ff
(cd 04-remote-one-ahead-ff
  echo "There are no unpushed local commits, the remote is one ahead (FF)" >.git/description
  commit init && setup_target_to_match_main
  git checkout -b A
    commit A1
    git checkout -b soon-A-remote
    commit A2
    setup_remote_tracking soon-A-remote A "move"

  git checkout A
  create_workspace_commit_once A
)

cp -R 04-remote-one-ahead-ff 04.5-remote-one-ahead-ff-merge
(cd 04.5-remote-one-ahead-ff-merge
  echo "Remote origin/A is merged back (fast-forward), bringing all into the target branch" >.git/description
  git checkout main
    git merge origin/A
    git rev-parse @ >.git/refs/remotes/origin/main

  git checkout gitbutler/workspace
)