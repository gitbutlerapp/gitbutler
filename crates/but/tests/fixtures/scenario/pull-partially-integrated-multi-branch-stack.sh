#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen

commit-file M
setup_target_to_match_main
git remote set-url origin .
git update-ref refs/heads/base main

git checkout -b C main
commit-file C

git checkout -b A
commit-file A

create_workspace_commit_once A

git checkout main
git merge --no-ff -m "merge C" C
commit-file upstream
git update-ref refs/remotes/origin/main main

git checkout gitbutler/workspace
