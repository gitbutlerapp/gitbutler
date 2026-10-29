#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# Single-branch mode with HEAD checked out directly on branch A. A has two
# local commits and neither commit is historically integrated into the target.
# origin/main advances with a single squash commit that reproduces A's final
# tree without using A's commits as ancestors. Review integration hints can
# still identify A's head as merged, so upstream integration should replace the
# direct checkout branch with the advanced target.
git init
commit M1
setup_target_to_match_main

git checkout -b A
commit-file A1.txt A1
commit-file A2.txt A2

git checkout -b upstream-main main
echo A1 >A1.txt
echo A2 >A2.txt
git add A1.txt A2.txt
git commit -m "squash A"
git update-ref refs/remotes/origin/main upstream-main

git checkout A
git branch -D upstream-main
