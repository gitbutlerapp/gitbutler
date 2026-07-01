#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit, with two stacks inside:
# - One stack `A` with file-a changes
# - Another stack with `B` introducing file-b and `C` introducing file-c
git init

echo base >base
git add base
git commit -m M
setup_target_to_match_main

git branch B
git checkout -b A
  echo a >file-a
  git add file-a
  git commit -m A
git checkout B
  echo b >file-b
  git add file-b
  git commit -m B
git checkout B -b C
  echo c >file-c
  git add file-c
  git commit -m C
create_workspace_commit_once A C
