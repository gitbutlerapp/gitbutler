#!/usr/bin/env bash

### Description
# One stack with two commits that edit the same line of the same file, so
# reordering or discarding them produces conflicted commits.

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen

echo base >file.txt
git add file.txt
git commit -m "base"
setup_target_to_match_main

git checkout -b A
echo one >file.txt
git add file.txt
git commit -m "set one"
echo two >file.txt
git add file.txt
git commit -m "set two"

create_workspace_commit_once A
