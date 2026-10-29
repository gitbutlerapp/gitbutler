#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# Branch B with new commit B1 is stacked on branch A with commits A2 and A1, all above
# old target M1. The target ref gained a single squash commit whose tree contains only the
# A1+A2 changes (as a squash-merge of A alone would produce), plus follow-up commit X on
# top. Local `main` stays at M1, the old target.
git init
commit-file M1.txt M1
setup_target_to_match_main

git checkout -b A
commit-file A1.txt A1
commit-file A2.txt A2
git checkout -b B
commit-file B1.txt B1

squashed=$(git commit-tree -p main -m "A1 + A2 (#1)" 'A^{tree}')
git checkout --detach "$squashed"
commit-file X.txt X
git update-ref refs/remotes/origin/main HEAD

git checkout B
create_workspace_commit_once B
