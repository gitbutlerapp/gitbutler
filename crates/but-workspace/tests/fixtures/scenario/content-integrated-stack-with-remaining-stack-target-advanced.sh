#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

# Stack A is content-integrated into the target ref with different commit IDs,
# while independent stack B remains in the workspace. Integrating A should
# remove it without reparenting the workspace commit to the target ref.
git init
commit-file C.txt C
setup_target_to_match_main

git checkout -b A
commit-file A.txt A

git checkout main
git checkout -b B
commit-file B.txt B

create_workspace_commit_once A B

git checkout main
git cherry-pick A
commit-file X.txt X
git update-ref refs/remotes/origin/main main
git checkout gitbutler/workspace
