#!/bin/bash

# Move a branch ref to a NEW commit that conflicts with the target base tree.
# This simulates an external force-push that introduces conflicting content.
#
# Usage: move-stack-ref-to-conflicting-commit.sh <branch-name> <directory>
#
# Creates a commit (not on any branch) whose tree conflicts with a_file at
# the base, then moves the given ref to point at it.

set -euo pipefail

BRANCH_NAME="$1"
DIR="$2"

echo "Moving ref '$BRANCH_NAME' to a conflicting commit in $DIR"

pushd "$DIR"
  BASE_OID="$(git rev-parse origin/master)"

  # Create a conflicting blob: overwrite a_file with different content.
  CONFLICT_BLOB="$(echo 'CONFLICTING CONTENT' | git hash-object -w --stdin)"

  # Build a tree that replaces a_file with the conflicting blob.
  BASE_TREE="$(git rev-parse "${BASE_OID}^{tree}")"
  NEW_TREE="$(git ls-tree "$BASE_TREE" | sed "s|[0-9a-f]\{40\}\ta_file|${CONFLICT_BLOB}\ta_file|" | git mktree)"

  # Create a commit with this tree, parented on the base.
  NEW_COMMIT="$(git commit-tree "$NEW_TREE" -p "$BASE_OID" -m 'external: conflicting change to a_file')"

  git update-ref "refs/heads/$BRANCH_NAME" "$NEW_COMMIT"
  echo "Updated refs/heads/$BRANCH_NAME -> $NEW_COMMIT (conflicts with base)"
popd
