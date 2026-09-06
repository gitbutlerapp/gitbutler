#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit over two stacks:
# - Stack 1 interleaves empty branches with commit owners:
#   A (file-a) <- e1 (empty) <- B (file-b), with e2 and e3 both empty on B's tip.
# - Stack 2 is a single commit-owning branch F (file-f).
git init
echo base >base
git add base
git commit -m M
setup_target_to_match_main

git checkout -b A
  echo a >file-a
  git add file-a
  git commit -m A
git branch e1
git checkout -b B
  echo b >file-b
  git add file-b
  git commit -m B
git branch e2
git branch e3

git checkout main
git checkout -b F
  echo f >file-f
  git add file-f
  git commit -m F

create_workspace_commit_once e3 F
