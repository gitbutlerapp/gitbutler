#!/usr/bin/env bash

### General Description

# Disjoint scenarios related to remote branches that have been rebased and merged.
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init 01-one-rewritten-one-local-after-push
(cd 01-one-rewritten-one-local-after-push
  echo "two local commits pushed to a remote, then rebased onto target.

The branch should then be considered integrated" >.git/description
  commit init && setup_target_to_match_main
  git checkout -b A
    echo A-new >file && git add file && git commit -m A1
    echo A-new >other && git add other && git commit -m A2
    git rev-parse @ >.git/refs/remotes/origin/A

  git checkout main
    commit M1
    git cherry-pick :/A1
    git cherry-pick :/A2
    echo M-change >>file && git commit -am M2
    echo M-change >>other && git commit -am M3
    git rev-parse @ >.git/refs/remotes/origin/main

  git checkout A
  create_workspace_commit_once A
)

git init 01-with-local-amended-after-integration
(cd 01-with-local-amended-after-integration
  echo "two local commits pushed to a remote, then rebased onto target, and local amended

The branch should then *not* be considered integrated anymore as A2 has changed" >.git/description
  commit init && setup_target_to_match_main
  git checkout -b A
    echo A-new >file && git add file && git commit -m A1
    echo A-new >other-squashed && git add other-squashed && git commit -m A2
    add_change_id_to_given_commit 2 @ | write_ref_safely .git/refs/heads/A
    git rev-parse @ >.git/refs/remotes/origin/A

  git checkout main
    # assure the time and message based check won't match.
    tick
    commit M1
    git cherry-pick :/A1
    # instead of cherry-picking, we create the 'original' version of the commit
    # and assign it the right change-id.
    echo A-new >other && git add other && git commit -m A2
    add_change_id_to_given_commit 2 @ | write_ref_safely .git/refs/heads/main

    echo M-change >>file && git commit -am M2
    echo M-change >>other && git commit -am M3
    git rev-parse @ >.git/refs/remotes/origin/main

  git checkout A
  create_workspace_commit_once A
)

git init 01-rewritten-local-commit-is-paired-with-remote
(cd 01-rewritten-local-commit-is-paired-with-remote
  echo "two local commits pushed to a remote, then changed locally.

One is changed locally and matched by message, the other one is matched by change-id
as the content is too different." >.git/description
  commit init && setup_target_to_match_main
  git checkout -b A
    git branch soon-A-remote
    echo A-new >file-different && git add file-different && git commit -m A1
    echo A-new >other-different && git add other-different && git commit -m A2
    add_change_id_to_given_commit 2 @ | write_ref_safely .git/refs/heads/A

  git checkout soon-A-remote
    # Here we want to the time and author check to match.
    echo A-new >file && git add file && git commit -m A1
    # assure the time and message based check won't match, so we get the change-id
    tick
    echo A-new >other && git add other && git commit -m A2
    add_change_id_to_given_commit 2 @ | write_ref_safely .git/refs/heads/soon-A-remote
    setup_remote_tracking soon-A-remote A "move"

  git checkout A
  create_workspace_commit_once A
)

git init 01-one-rewritten-one-local-after-push-author-date-change
(cd 01-one-rewritten-one-local-after-push-author-date-change
  echo "two local commits pushed to a remote, then rebased onto target, but with the author date adjusted.

This prevents quick-checks to work." >.git/description
  commit init && setup_target_to_match_main
  git checkout -b A
    echo A-new >file && git add file && git commit -m A1
    echo A-new >other && git add other && git commit -m A2
    git rev-parse @ >.git/refs/remotes/origin/A

  git checkout main
    commit M1
    tick
    git cherry-pick :/A1
    tick
    git cherry-pick :/A2
    echo M-change >>file && git commit -am M2
    echo M-change >>other && git commit -am M3
    git rev-parse @ >.git/refs/remotes/origin/main

  git checkout A
  create_workspace_commit_once A
)
