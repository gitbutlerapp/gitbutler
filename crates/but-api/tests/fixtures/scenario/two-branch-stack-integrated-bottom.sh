#!/usr/bin/env bash

# A workspace with one stack of two branches, where the bottom branch's
# content already exists upstream (making it "integrated").
#
# main: M1 -> M2 (M2 adds branch1-file.txt with same content as branch1)
# origin/main points to M2
# branch1: M1 -> "branch1: first commit" (adds branch1-file.txt)
# branch3: branch1 -> "branch3: first commit" (adds branch3-file.txt)
# gitbutler/workspace: on top of branch3
#
# Tests set default_target.sha = M1, so origin/main (at M2) is ahead.

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init -b main

commit M1
git branch M1

echo "branch1 work" > branch1-file.txt
git add branch1-file.txt
git commit -m "upstream: merge branch1"

setup_target_to_match_main

git checkout M1
git checkout -b branch1
  echo "branch1 work" > branch1-file.txt
  git add branch1-file.txt
  git commit -m "branch1: first commit"
git checkout -b branch3
  echo "branch3 work" > branch3-file.txt
  git add branch3-file.txt
  git commit -m "branch3: first commit"

create_workspace_commit_once branch3
