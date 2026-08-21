#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# Two independent stacks on a shared base, with upstream advanced so that
# pulling conflicts stack A's commit.
# Stack A changes `file.txt`, which upstream also changes.
# Stack B renames `shared.txt` to `renamed.txt` without touching its content.
# Resolving A's conflicted commit while also modifying `shared.txt` makes the
# rewritten A tip and B's tip merge only when rename detection is enabled.
git-init-frozen

echo base > file.txt
cat > shared.txt << 'CONTENT'
line 1
line 2
line 3
line 4
line 5
CONTENT
git add file.txt shared.txt
git commit -m "base"
git update-ref refs/heads/base HEAD

git checkout -b A
echo change-on-A > file.txt
git add file.txt
git commit -m "A-change"

git checkout -b B main
git mv shared.txt renamed.txt
git commit -m "B-rename-shared"

git checkout main
echo change-on-main > file.txt
git add file.txt
git commit -m "main-change"
setup_target_to_match_main

git checkout A
create_workspace_commit_once A B
