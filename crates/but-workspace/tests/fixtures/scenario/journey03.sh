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
