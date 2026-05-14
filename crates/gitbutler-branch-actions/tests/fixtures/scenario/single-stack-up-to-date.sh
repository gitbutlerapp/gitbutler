#!/usr/bin/env bash
set -eu -o pipefail

# A single stack where origin/master has not advanced past the stack's
# fork point. The behind count should be 0.
#
# History (after setup):
#   origin/master = base
#   Stack A forks from base (0 commits behind)
#
# HEAD is on branch A so set_base_branch picks it up as a workspace stack.

git init -b master local
(cd local
  git config commit.gpgsign false
  git config user.name gitbutler-test
  git config user.email gitbutler-test@example.com
  git config gitbutler.storagePath gitbutler

  echo "base" >file && git add . && git commit -m "base"

  # Stack A forks from base (same as origin/master)
  git checkout -b A
  echo "A-work" >a-file && git add . && git commit -m "A: work"

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
