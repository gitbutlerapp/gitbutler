#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

# The workspace commit has the anonymous commit C2 (anonymous segment) as first parent and stack A as
# second parent. The target ref advances to X without integrating A. Integrating
# upstream should update only the target parent and keep A in the workspace.
git init
commit-file C.txt C1
setup_target_to_match_main
git checkout -b target-sha


git checkout target-sha -b A
commit-file A.txt A

git checkout --detach target-sha
commit-file C.txt C2

git checkout -b gitbutler/workspace
git merge --no-ff -m "GitButler Workspace Commit" A

git checkout main
commit-file X.txt X
git update-ref refs/remotes/origin/main main
git checkout gitbutler/workspace
