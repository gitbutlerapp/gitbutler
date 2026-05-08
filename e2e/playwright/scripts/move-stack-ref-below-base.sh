#!/bin/bash

# Move the given stack's ref to the target base commit (i.e. below the workspace),
# simulating an external tool corrupting the ref.
#
# Usage: move-stack-ref-below-base.sh <branch-name> <directory>
#
# This must be run AFTER the workspace is already set up and the branch is applied.

set -euo pipefail

BRANCH_NAME="$1"
DIR="$2"

echo "Moving ref '$BRANCH_NAME' below base in $DIR"

pushd "$DIR"
  # Find the target base commit (origin/master tip).
  BASE_OID="$(git rev-parse origin/master)"
  echo "Target base OID: $BASE_OID"

  # Force-update the stack ref to point at the base commit.
  git update-ref "refs/heads/$BRANCH_NAME" "$BASE_OID"
  echo "Updated refs/heads/$BRANCH_NAME -> $BASE_OID"
popd
