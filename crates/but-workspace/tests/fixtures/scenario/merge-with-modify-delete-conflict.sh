#!/usr/bin/env bash

set -eu -o pipefail
source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# Two independent stacks branching from main.
# Stack "modify-shared" modifies a file ("shared-file") that also exists on main.
# Stack "delete-shared" deletes that same "shared-file" AND also deletes a
# second file ("also-deleted") that the other stack doesn't touch.
#
# This reproduces a cross-stack modify/delete conflict: when merging the
# stacks into a workspace tree, the modify/delete on "shared-file" must NOT
# prevent other changes from "delete-shared" (like the deletion of
# "also-deleted") from being applied.
git init

echo "base content" > shared-file && git add shared-file
echo "keep this" > also-deleted && git add also-deleted
echo "untouched" > untouched && git add untouched
git commit -m "init"

# Stack A: modifies shared-file (but doesn't touch also-deleted).
git checkout -b modify-shared main
  echo "modified content" > shared-file && git add shared-file
  git commit -m "modify shared-file"

# Stack B: deletes shared-file AND also-deleted.
git checkout -b delete-shared main
  git rm shared-file && git rm also-deleted
  git commit -m "delete shared-file and also-deleted"

# Build a workspace merge commit manually via commit-tree.
# We can't use `git merge` because it would fail on the modify/delete
# conflict. GitButler constructs the workspace commit programmatically
# anyway, so we just need a merge commit with the right parents.
# Use modify-shared's tree as a stand-in (the test rebuilds the tree itself).
git checkout -b gitbutler/workspace
workspace_tree=$(git rev-parse modify-shared^{tree})
workspace_commit=$(git commit-tree "$workspace_tree" \
  -p modify-shared \
  -p delete-shared \
  -m "GitButler Workspace Commit")
git update-ref HEAD "$workspace_commit"
