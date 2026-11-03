#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

# One branch in the workspace with two commits that edit the same line, so that
# pulling the target conflicts both of them on shared.txt.
git-init-frozen

echo base >shared.txt
git add shared.txt
git commit -m "base"
setup_target_to_match_main
git remote set-url origin .

git checkout -b A
echo older >shared.txt
git add shared.txt
git commit -m "older change"
echo newer >shared.txt
git add shared.txt
git commit -m "newer change"

create_workspace_commit_once A

git checkout main
echo upstream >shared.txt
git add shared.txt
git commit -m "upstream change"
git update-ref refs/remotes/origin/main main

git checkout gitbutler/workspace
