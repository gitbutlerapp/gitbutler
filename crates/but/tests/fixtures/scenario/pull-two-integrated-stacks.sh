#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen

commit-file M
setup_target_to_match_main
git remote set-url origin .

git checkout -b A main
commit-file A

git checkout -b B main
commit-file B

create_workspace_commit_once A B

git checkout main
git merge --no-ff -m "merge A" A
git merge --no-ff -m "merge B" B
commit-file upstream
git update-ref refs/remotes/origin/main main

git checkout gitbutler/workspace
