#!/usr/bin/env bash

### General Description

# Various stages through a typical user journey. Ideally complete enough to test everything that matters to us.
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
    commit A2
  git checkout soon-A-remote
    echo A-new >file && git add file && git commit -m "A1 (same but different)"
    setup_remote_tracking soon-A-remote A "move"

  git checkout A
  create_workspace_commit_once A
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
