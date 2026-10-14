#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen
commit-file M
setup_target_to_match_main

git checkout -b A main
commit-file A1
git update-ref refs/remotes/origin/A A

git checkout -b B main
commit-file B1

git checkout main
create_workspace_commit_once A B

# On the remote, a collaborator merges a newer main into A, then commits on top.
git checkout -b newer-main main
commit-file main2
commit-file main3
git checkout -b remote-A A
git merge --no-ff -m "Merge newer main into A" newer-main
commit-file A2-by-collaborator
git update-ref refs/remotes/origin/A remote-A

git checkout gitbutler/workspace
git branch -D newer-main remote-A
