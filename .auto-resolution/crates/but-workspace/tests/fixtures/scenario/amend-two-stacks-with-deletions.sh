#!/usr/bin/env bash

# Scenario: Two independent branches (A and B) in the workspace.
# Branch A has a commit that adds a-file.txt.
# Branch B has a commit that adds b-file.txt.
# The workspace commit merges both.
#
# Uncommitted state:
#   - a-file.txt is modified
#   - b-file.txt is deleted
#
# Used to test that amending a-file.txt into A's commit does NOT
# discard the uncommitted deletion of b-file.txt.

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
commit M
setup_target_to_match_main

git branch B
git checkout -b A
  echo "content-a" > a-file.txt
  git add a-file.txt
  git commit -m "add a-file"
git checkout B
  echo "content-b" > b-file.txt
  git add b-file.txt
  git commit -m "add b-file"
create_workspace_commit_once A B

# Uncommitted changes: modify a-file, delete b-file
echo "modified-a" > a-file.txt
rm b-file.txt
