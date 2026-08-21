#!/usr/bin/env bash

set -eu -o pipefail
source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# Two independent branches editing the same file with non-overlapping changes.
# This triggers a false merge conflict with Myers diff algorithm due to
# split hunks with empty insertions.
# See: https://github.com/GitoxideLabs/gitoxide/issues/2475
#
# Base file has:
#   alpha_x
#   (blank)
#   bravo_x
#   charlie_x
#   (blank)
#
# Branch "delete-alpha" removes alpha_x (replaces with blank) and trailing blank.
# Branch "delete-bravo" removes bravo_x.
# These are non-overlapping edits that git merge-file resolves cleanly,
# but Myers diff produces a false conflict.

git init

# Create the base content on main
printf 'alpha_x\n\nbravo_x\ncharlie_x\n\n' > shared-file
git add shared-file
git commit -m "base: shared-file with alpha, bravo, charlie"

git branch gitbutler/workspace

# Branch A: delete alpha_x (replace with blank line) and remove trailing blank
git checkout -b delete-alpha main
printf '\n\nbravo_x\ncharlie_x\n' > shared-file
git add shared-file
git commit -m "delete alpha_x from shared-file"

# Branch B: delete bravo_x
git checkout -b delete-bravo main
printf 'alpha_x\n\ncharlie_x\n\n' > shared-file
git add shared-file
git commit -m "delete bravo_x from shared-file"
