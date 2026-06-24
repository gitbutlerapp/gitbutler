#!/usr/bin/env bash

set -eu -o pipefail
source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# Similar to myers-false-conflict-same-file but with multiple sequential commits
# per branch, to test squash and amend operations in the context of false conflicts.
#
# Base file:
#   alpha_x
#   (blank)
#   bravo_x
#   charlie_x
#   (blank)
#
# Branch "edit-alpha" has two commits:
#   1. Adds a comment line at the top
#   2. Deletes alpha_x (replaces with blank) and removes trailing blank
#
# Branch "edit-bravo" has two commits:
#   1. Adds a comment line at the bottom
#   2. Deletes bravo_x
#
# The final state of each branch triggers the Myers false conflict.
# Squashing the two commits in either branch should also work but may
# trigger merge issues during the rebase.

git init

# Create the base content on main
printf 'alpha_x\n\nbravo_x\ncharlie_x\n\n' > shared-file
git add shared-file
git commit -m "base: shared-file with alpha, bravo, charlie"

git branch gitbutler/workspace

# Branch A: two sequential commits
git checkout -b edit-alpha main

# First commit: add a header comment (doesn't conflict with anything)
printf '# header\nalpha_x\n\nbravo_x\ncharlie_x\n\n' > shared-file
git add shared-file
git commit -m "add header to shared-file"

# Second commit: delete alpha_x and trailing blank
printf '# header\n\n\nbravo_x\ncharlie_x\n' > shared-file
git add shared-file
git commit -m "delete alpha_x from shared-file"

# Branch B: two sequential commits
git checkout -b edit-bravo main

# First commit: add a footer comment (doesn't conflict with anything)
printf 'alpha_x\n\nbravo_x\ncharlie_x\n\n# footer\n' > shared-file
git add shared-file
git commit -m "add footer to shared-file"

# Second commit: delete bravo_x
printf 'alpha_x\n\ncharlie_x\n\n# footer\n' > shared-file
git add shared-file
git commit -m "delete bravo_x from shared-file"
