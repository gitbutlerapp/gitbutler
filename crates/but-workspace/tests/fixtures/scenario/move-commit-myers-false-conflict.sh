#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# Two stacked commits on a single branch, both editing a file with the blank-line
# pattern that triggers the Myers diff false conflict (GitoxideLabs/gitoxide#2475).
# An empty second branch is set up as a move target.
#
# Base file: alpha_x\n\nbravo_x\ncharlie_x\n\n
# Commit 1 (delete-alpha): deletes alpha_x (replaces with blank, removes trailing blank)
# Commit 2 (delete-bravo): deletes bravo_x
#
# Moving commit 2 to the empty branch cherry-picks it onto main, producing a 3-way
# merge whose base tree has the blank-line structure that triggers the Myers bug.

git init

printf 'alpha_x\n\nbravo_x\ncharlie_x\n\n' > shared-file
git add shared-file
git commit -m "M"
setup_target_to_match_main

git branch B

git checkout -b A
  printf '\n\nbravo_x\ncharlie_x\n' > shared-file
  git add shared-file
  git commit -m "delete alpha_x"

  printf '\n\ncharlie_x\n' > shared-file
  git add shared-file
  git commit -m "delete bravo_x"

git checkout B
create_workspace_commit_once A B
