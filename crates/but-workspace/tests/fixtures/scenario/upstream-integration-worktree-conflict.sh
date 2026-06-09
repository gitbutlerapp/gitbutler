#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A workspace branch rebases cleanly onto the updated target, but a dirty
# worktree edit to shared.txt would conflict when applied onto the new workspace
# head.
git init
echo base >shared.txt
git add shared.txt
git commit -m M1
setup_target_to_match_main

git checkout -b A
commit-file A.txt A1

create_workspace_commit_once A

git checkout main
echo upstream >shared.txt
git add shared.txt
git commit -m "update shared upstream"
git update-ref refs/remotes/origin/main main

git checkout gitbutler/workspace
