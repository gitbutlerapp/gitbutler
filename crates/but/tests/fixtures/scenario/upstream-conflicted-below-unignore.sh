#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# Branch A's first commit and the target both change `file.txt`, so integrating the target
# conflicts it. A's second commit stops ignoring `build.log`, which the base ignores.

git-init-frozen

echo build.log >.gitignore
echo base >file.txt
git add .gitignore file.txt
git commit -m "base"
git update-ref refs/heads/base HEAD

git checkout -b A
echo change-on-A >file.txt
git add file.txt
git commit -m "A-change"
git rm -q .gitignore
git commit -m "stop ignoring build.log"

git checkout main
echo change-on-main >file.txt
git add file.txt
git commit -m "main-change"
setup_target_to_match_main

git checkout A
create_workspace_commit_once A
