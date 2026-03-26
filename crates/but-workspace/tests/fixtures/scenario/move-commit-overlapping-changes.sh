#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# Two stacked commits on a single branch, both editing the SAME line of a file.
# An empty second branch is set up as a move target.
#
# Base file: alpha\nbravo\ncharlie\n
# Commit 1: changes bravo → bravo_modified
# Commit 2: changes bravo_modified → bravo_replaced
#
# Moving commit 2 to the empty branch cherry-picks it onto main. The 3-way merge
# has a genuine conflict: base has bravo_modified, ours has bravo, theirs has
# bravo_replaced — all three differ on the same line.

git init

printf 'alpha\nbravo\ncharlie\n' > shared-file
git add shared-file
git commit -m "M"
setup_target_to_match_main

git branch B

git checkout -b A
  printf 'alpha\nbravo_modified\ncharlie\n' > shared-file
  git add shared-file
  git commit -m "modify bravo"

  printf 'alpha\nbravo_replaced\ncharlie\n' > shared-file
  git add shared-file
  git commit -m "replace bravo"

git checkout B
create_workspace_commit_once A B
