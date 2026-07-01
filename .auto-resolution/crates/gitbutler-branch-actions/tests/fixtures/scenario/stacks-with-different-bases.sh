#!/usr/bin/env bash
set -eu -o pipefail

# Two stacks that fork from different points on master, with origin/master
# advanced past both. This creates a scenario where each stack has
# a different number of "behind" commits relative to the target.
#
# History (after setup):
#   origin/master = M3 -> M2 -> M1 -> base
#   Stack A forks from base   (3 commits behind)
#   Stack C forks from M2     (1 commit behind)
#
# HEAD is on branch A so set_base_branch picks it up as a workspace stack.
# Branch C can then be applied separately.

git init -b master local
(cd local
  git config commit.gpgsign false
  git config user.name gitbutler-test
  git config user.email gitbutler-test@example.com
  git config gitbutler.storagePath gitbutler

  echo "base" >file && git add . && git commit -m "base"

  # Stack A forks from base
  git checkout -b A
  echo "A-work" >a-file && git add . && git commit -m "A: work"

  # Advance master to M1, M2, M3
  git checkout master
  echo "M1" >>file && git add . && git commit -m "M1"
  echo "M2" >>file && git add . && git commit -m "M2"

  # Stack C forks from M2
  git checkout -b C
  echo "C-work" >c-file && git add . && git commit -m "C: work"

  git checkout master
  echo "M3" >>file && git add . && git commit -m "M3"

  # Leave HEAD on A so set_base_branch picks it up
  git checkout A
)

git init --bare remote
(cd local
  remote_path="$(cd ../remote && pwd)"
  git remote add origin "$remote_path"
  git push origin master
  git fetch origin '+refs/heads/*:refs/remotes/origin/*'
)
