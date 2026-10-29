#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A three-branch stack above old target M1: branch A (bottom, commits A1 and A2),
# branch B (middle, adds Y.txt), branch C (top, deletes Y.txt again) - so the tree at
# C's tip equals the tree at A's tip. The target ref gained a squash commit of A's
# changes only, plus follow-up commit X. B and C never landed and must survive.
git init
commit-file M1.txt M1
setup_target_to_match_main

git checkout -b A
commit-file A1.txt A1
commit-file A2.txt A2
git checkout -b B
commit-file Y.txt Y
git checkout -b C
git rm -q Y.txt
tick
git commit -qm "delete Y"

squashed=$(git commit-tree -p main -m "A1 + A2 (#1)" 'A^{tree}')
git checkout --detach "$squashed"
commit-file X.txt X
git update-ref refs/remotes/origin/main HEAD

git checkout C
create_workspace_commit_once C
