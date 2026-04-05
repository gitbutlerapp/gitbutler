#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# Two stacked commits on a single branch editing different parts of the same file.
# An empty second branch is set up as a move target.
#
# Base file: alpha\nbravo\ncharlie\n
# Commit 1: adds "first" at the top → first\nalpha\nbravo\ncharlie\n
# Commit 2: adds "last" at the bottom → first\nalpha\nbravo\ncharlie\nlast\n
#
# Moving commit 2 to the empty branch cherry-picks it onto main. The 3-way merge:
#   base = first\nalpha\nbravo\ncharlie\n  (parent of commit 2)
#   ours = alpha\nbravo\ncharlie\n          (main)
#   theirs = first\nalpha\nbravo\ncharlie\nlast\n  (commit 2)
#
# base→ours removes "first" at top; base→theirs adds "last" at bottom.
# These are non-overlapping edits that should merge cleanly.

git init

printf 'alpha\nbravo\ncharlie\n' > shared-file
git add shared-file
git commit -m "M"
setup_target_to_match_main

git branch B

git checkout -b A
  printf 'first\nalpha\nbravo\ncharlie\n' > shared-file
  git add shared-file
  git commit -m "add first at top"

  printf 'first\nalpha\nbravo\ncharlie\nlast\n' > shared-file
  git add shared-file
  git commit -m "add last at bottom"

git checkout B
create_workspace_commit_once A B
