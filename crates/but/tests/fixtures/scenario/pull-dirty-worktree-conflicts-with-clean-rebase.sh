#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

# A workspace branch rebases cleanly onto the updated target (it only adds a new
# file), but a dirty worktree edit to shared.txt conflicts when applied onto the
# new workspace head, which now carries the upstream change to shared.txt.

git-init-frozen

echo base >shared.txt
git add shared.txt
git commit -m "base"
setup_target_to_match_main
git remote set-url origin .

git checkout -b A
echo a >A.txt
git add A.txt
git commit -m "add A"

create_workspace_commit_once A

git checkout main
echo upstream >shared.txt
git add shared.txt
git commit -m "upstream change"
git update-ref refs/remotes/origin/main main

git checkout gitbutler/workspace
