#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit, with two deeper stacks inside:
# - One stack `A -> D` with file-a and file-d changes
# - Another stack `B -> C -> E` with file-b, file-c, and file-e changes
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
git checkout A -b D
  echo d >file-d
  git add file-d
  git commit -m D
git checkout B
  echo b >file-b
  git add file-b
  git commit -m B
git checkout B -b C
  echo c >file-c
  git add file-c
  git commit -m C
git checkout C -b E
  echo e >file-e
  git add file-e
  git commit -m E
create_workspace_commit_once D E
