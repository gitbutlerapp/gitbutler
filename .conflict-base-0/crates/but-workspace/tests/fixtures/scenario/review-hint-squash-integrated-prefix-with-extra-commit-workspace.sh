#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A managed workspace contains one stack branch A with three local commits. None
# of the local commits are historically integrated into the target. origin/main
# advances with a single squash commit that reproduces only A1 and A2, while A3
# remains local work above the squashed review head. Review integration hints
# point at A2, so upstream integration should drop the integrated prefix and
# keep A3 rebased onto the squashed target tip.
git init
commit M1
setup_target_to_match_main

git checkout -b A
commit-file A1.txt A1
commit-file A2.txt A2
commit-file A3.txt A3

create_workspace_commit_once A

git checkout -b upstream-main main
echo A1 >A1.txt
echo A2 >A2.txt
git add A1.txt A2.txt
git commit -m "squash A1 and A2"
git update-ref refs/remotes/origin/main upstream-main

git checkout gitbutler/workspace
git branch -D upstream-main
