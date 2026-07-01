#!/bin/bash

set -eu -o pipefail

# This fixture creates a merge where the first parent has an earlier
# committer timestamp than the second parent. This causes the but-graph
# traversal queue sort (which processes younger/newer commits first) to
# process the second parent before the first parent, leading to edges
# being created in an order that doesn't match parent_ids.

git init

# Create base commit
GIT_COMMITTER_DATE="2020-01-01T00:00:00Z" GIT_AUTHOR_DATE="2020-01-01T00:00:00Z" \
  git commit --allow-empty -m "base" && git tag base

# Create the first-parent branch with an OLD commit
git checkout -b first-parent
GIT_COMMITTER_DATE="2020-01-02T00:00:00Z" GIT_AUTHOR_DATE="2020-01-02T00:00:00Z" \
  git commit --allow-empty -m "old commit on first-parent"

# Create the second-parent branch with NEWER commits (higher gen AND timestamp)
git checkout -b second-parent main
GIT_COMMITTER_DATE="2024-06-01T00:00:00Z" GIT_AUTHOR_DATE="2024-06-01T00:00:00Z" \
  git commit --allow-empty -m "new commit 1 on second-parent"
GIT_COMMITTER_DATE="2024-06-02T00:00:00Z" GIT_AUTHOR_DATE="2024-06-02T00:00:00Z" \
  git commit --allow-empty -m "new commit 2 on second-parent"
GIT_COMMITTER_DATE="2024-06-03T00:00:00Z" GIT_AUTHOR_DATE="2024-06-03T00:00:00Z" \
  git commit --allow-empty -m "new commit 3 on second-parent"

# Merge second-parent into first-parent.
# First parent = first-parent branch (old, low timestamp)
# Second parent = second-parent branch (new, high timestamp)
git checkout first-parent
GIT_COMMITTER_DATE="2024-07-01T00:00:00Z" GIT_AUTHOR_DATE="2024-07-01T00:00:00Z" \
  git merge --no-ff second-parent -m "merge second-parent into first-parent"

# Add a commit on top so the merge is not HEAD (it's at index 1 in the segment)
GIT_COMMITTER_DATE="2024-07-02T00:00:00Z" GIT_AUTHOR_DATE="2024-07-02T00:00:00Z" \
  git commit --allow-empty -m "commit on top of merge"
