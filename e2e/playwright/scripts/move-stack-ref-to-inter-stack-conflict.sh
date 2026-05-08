#!/bin/bash

# Move a branch ref to a new commit that conflicts with ANOTHER applied stack's changes.
# This simulates a force-push that creates an inter-stack merge conflict.
#
# Usage: move-stack-ref-to-inter-stack-conflict.sh <branch-name> <directory>
#
# Creates a commit that appends different content to a_file at the same position
# where branch1 also appends. This forces a true 3-way merge conflict when both
# stacks are applied to the workspace.

set -euo pipefail

BRANCH_NAME="$1"
DIR="$2"

echo "Moving ref '$BRANCH_NAME' to an inter-stack conflicting commit in $DIR"

pushd "$DIR"
  BASE_OID="$(git rev-parse origin/master)"

  # branch1 appends "branch1 commit 1" etc. to a_file starting at line 4.
  # We also append at line 4, creating overlapping changes → real merge conflict.
  BASE_TREE="$(git rev-parse "${BASE_OID}^{tree}")"

  # Get base a_file content and append conflicting lines at the same position.
  BASE_A_FILE="$(git cat-file -p "${BASE_TREE}:a_file")"
  CONFLICT_BLOB="$(printf '%s\nINTER-STACK CONFLICT LINE\n' "$BASE_A_FILE" | git hash-object -w --stdin)"

  # Build a tree with the modified a_file.
  NEW_TREE="$(git ls-tree "$BASE_TREE" | sed "s|[0-9a-f]\{40\}\ta_file|${CONFLICT_BLOB}\ta_file|" | git mktree)"

  # Create a commit with this tree, parented on the base.
  NEW_COMMIT="$(git commit-tree "$NEW_TREE" -p "$BASE_OID" -m 'external: inter-stack conflicting change to a_file')"

  git update-ref "refs/heads/$BRANCH_NAME" "$NEW_COMMIT"
  echo "Updated refs/heads/$BRANCH_NAME -> $NEW_COMMIT (inter-stack conflict)"
popd
