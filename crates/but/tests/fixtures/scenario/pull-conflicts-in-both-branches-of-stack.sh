#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen

echo base >bottom.txt
echo base >top.txt
git add bottom.txt top.txt
git commit -m "base"
setup_target_to_match_main
git remote set-url origin .

git checkout -b B
echo bottom >bottom.txt
git add bottom.txt
git commit -m "bottom change"

git checkout -b A
echo top >top.txt
git add top.txt
git commit -m "top change"

create_workspace_commit_once A

git checkout main
echo upstream >bottom.txt
echo upstream >top.txt
git add bottom.txt top.txt
git commit -m "upstream change"
git update-ref refs/remotes/origin/main main

git checkout gitbutler/workspace
