#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen

echo base > file.txt
git add file.txt
git commit -m "base"
git update-ref refs/heads/base HEAD

git checkout -b A

echo change-A > file-a.txt
git add file-a.txt
git commit -m "A-change"
A_COMMIT=$(git rev-parse HEAD)

git checkout -b B main

echo change-B > file-b.txt
git add file-b.txt
git commit -m "B-change"

# Create extra branches that sit on 'main' (at or below the target).
# These will be used to test pruning behavior — some registered in
# metadata (simulating auto-discovery), some not.
git checkout -b extra-untracked main
git checkout -b extra-untracked-2 main

git checkout main
git cherry-pick "$A_COMMIT"
echo main-advance > file-main.txt
git add file-main.txt
git commit -m "main-advance"
setup_target_to_match_main

git checkout B
create_workspace_commit_once A B extra-untracked extra-untracked-2
