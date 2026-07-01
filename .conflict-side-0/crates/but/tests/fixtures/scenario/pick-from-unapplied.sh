#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### Scenario for testing `but pick` command
# Creates:
# - One applied stack (applied-branch) with a commit
# - One unapplied branch (unapplied-branch) with two commits that can be picked

git-init-frozen
tick
commit-file M
setup_target_to_match_main

# Create the applied branch
git checkout -b applied-branch
tick
echo "applied content" > applied.txt
git add applied.txt
git commit -m "add applied.txt"

# Create an unapplied branch from main with commits to pick
git checkout -b unapplied-branch main
tick
echo "pick me first" > pickable1.txt
git add pickable1.txt
git commit -m "first pickable commit"

tick
echo "pick me second" > pickable2.txt
git add pickable2.txt
git commit -m "second pickable commit"

# Store the commit SHAs for easy access in tests
git update-ref refs/gitbutler/pickable-head HEAD
git update-ref refs/gitbutler/pickable-first HEAD~1

# Go back to applied-branch and create workspace
git checkout applied-branch
create_workspace_commit_once applied-branch
