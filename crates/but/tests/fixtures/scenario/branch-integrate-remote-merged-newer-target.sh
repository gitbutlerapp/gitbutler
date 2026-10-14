#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen
commit-file M
setup_target_to_match_main
git init --bare .git/remote.git
git remote set-url origin ./.git/remote.git

git checkout -b A main
commit-file A1

git checkout -b B main
commit-file B1

git checkout main
create_workspace_commit_once A B
git push origin main A

# On the remote, a collaborator merges a newer main into A, then commits on top.
git checkout -b newer-main main
commit-file main2
commit-file main3
git checkout -b remote-A A
git merge --no-ff -m "Merge newer main into A" newer-main
commit-file A2-by-collaborator
git push origin newer-main:main remote-A:A

# Only A was fetched: origin/main still points at the old main.
git update-ref refs/remotes/origin/main main

git checkout gitbutler/workspace
git branch -D newer-main remote-A
