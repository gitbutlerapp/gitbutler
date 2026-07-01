#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit with two independent file-backed stacks.
git init

echo base >base
git add base
git commit -m M
setup_target_to_match_main

git branch B
git checkout -b A
  echo A >A
  git add A
  git commit -m A
git checkout B
  echo B >B
  git add B
  git commit -m B
create_workspace_commit_once A B
