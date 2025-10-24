#!/bin/bash

# A fixture to create a repository with a lot of worktrees.
set -eu -o pipefail

git init prime
(cd prime
  git commit -m "init" --allow-empty
  git branch not-checked-out

  git worktree add ../worktree-one

  git worktree add ../worktree-without-checkout
    rm -Rf ../worktree-without-checkout
)
