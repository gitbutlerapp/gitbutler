#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen

echo base > file.txt
git add file.txt
git commit -m "base"

git checkout -b A

echo change-A > file-a.txt
git add file-a.txt
git commit -m "A-change"
A_COMMIT=$(git rev-parse HEAD)

git checkout -b B main

echo change-B > file-b.txt
git add file-b.txt
git commit -m "B-change"

git checkout main
git cherry-pick "$A_COMMIT"
echo main-advance > file-main.txt
git add file-main.txt
git commit -m "main-advance"
setup_target_to_match_main

git checkout B
create_workspace_commit_once A B
