#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

# Simulate the scenario where origin/master advances (via fetch) after the
# workspace commit was created. An old branch ref (`old-integrated`) ends up
# on a commit that is only reachable from the new target, not from the
# workspace commit.  It should NOT appear in any workspace stack.

git-init-frozen

echo base > file.txt
git add file.txt
git commit -m "base"

# --- stack A: one local commit ---
git checkout -b A main
echo change-A > file-a.txt
git add file-a.txt
git commit -m "A-change"

# --- stack B: one local commit ---
git checkout -b B main
echo change-B > file-b.txt
git add file-b.txt
git commit -m "B-change"

# Set up the target to match main at "base"
git checkout main
setup_target_to_match_main

# Create the workspace commit with A and B.
git checkout B
create_workspace_commit_once A B

# Now simulate what happens after a fetch: main advances with new commits,
# and an old branch ref sits on one of those commits.
git checkout main
echo first-advance > file-first.txt
git add file-first.txt
git commit -m "first-advance"

# Create old-integrated branch pointing at this commit on main.
# This simulates a branch that was merged (its tip is now on main's history).
git branch old-integrated HEAD

echo second-advance > file-second.txt
git add file-second.txt
git commit -m "second-advance"

# Update origin/main to point to the new main (simulating fetch)
git update-ref refs/remotes/origin/main HEAD

# Go back to workspace
git checkout gitbutler/workspace
