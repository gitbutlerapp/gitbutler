#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A managed workspace contains two independent single-branch stacks. Stack A
# has two local commits and stack B has one local commit. None of the stack
# commits are historically integrated into the target: origin/main advances with
# a single squash commit that reproduces stack A's final tree without using A's
# commits as ancestors. Review integration hints can still identify A's head as
# merged, so upstream integration should remove stack A while keeping stack B.
git init
commit M1
setup_target_to_match_main

git checkout -b A
commit-file A1.txt A1
commit-file A2.txt A2

git checkout main
git checkout -b B
commit-file B.txt B1

create_workspace_commit_once A B

git checkout -b upstream-main main
echo A1 >A1.txt
echo A2 >A2.txt
git add A1.txt A2.txt
git commit -m "squash A"
git update-ref refs/remotes/origin/main upstream-main

git checkout gitbutler/workspace
git branch -D upstream-main
