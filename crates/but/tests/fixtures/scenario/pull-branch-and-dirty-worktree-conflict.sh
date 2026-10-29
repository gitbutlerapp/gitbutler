#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen

printf 'base\nunchanged\n' >shared.txt
git add shared.txt
git commit -m "base"
setup_target_to_match_main
git remote set-url origin .

git checkout -b A
printf 'local\nunchanged\n' >shared.txt
git add shared.txt
git commit -m "local change"

create_workspace_commit_once A

git checkout main
printf 'upstream\nunchanged\n' >shared.txt
git add shared.txt
git commit -m "upstream change"
git update-ref refs/remotes/origin/main main

git checkout gitbutler/workspace
