#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen

echo base > file.txt
git add file.txt
git commit -m "base"

git checkout -b A

echo change-on-A > file.txt
git add file.txt
git commit -m "A-change"

git checkout main

echo change-on-main > file.txt
git add file.txt
git commit -m "main-change"
setup_target_to_match_main

git checkout A
create_workspace_commit_once A
