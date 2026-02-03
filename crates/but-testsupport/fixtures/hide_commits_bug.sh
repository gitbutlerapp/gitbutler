#!/usr/bin/env bash
# Creates a repository for testing the hide commits functionality in gix-traverse.
#
# This script creates a graph structure with multiple interesting tips and a
# hidden tip to test that commits reachable from the hidden tip are correctly
# excluded from traversal results.
#
# Graph structure:
#
#    base -- fork
#            / | \
#          i1  h1 i2
#          |   |   |
#          i3  h2 i4
#              |
#              h3 (hidden tip)
#
# When traversing from [i3, i4] while hiding h3:
# - fork and base are common ancestors of both interesting and hidden tips
# - fork and base should be hidden because they're reachable from h3
# - The test verifies that only i3, i1, i4, i2 are returned

set -eu -o pipefail

function tick () {
  if test -z "${tick+set}"; then
    tick=1112911993
  else
    tick=$(($tick + 60))
  fi
  GIT_COMMITTER_DATE="$tick -0700"
  GIT_AUTHOR_DATE="$tick -0700"
  export GIT_COMMITTER_DATE GIT_AUTHOR_DATE
}

git init -q
git config user.email "test@example.com"
git config user.name "Test User"

# Create base commit
tick
echo "base" > file.txt
git add file.txt
git commit -m "base"

# Create fork point
tick
echo "fork" > file.txt
git add file.txt
git commit -m "fork"

# Create the hidden branch FIRST (older timestamps)
tick
git checkout -b hidden-branch
echo "h1" > file.txt
git add file.txt
git commit -m "h1"

tick
echo "h2" > file.txt
git add file.txt
git commit -m "h2"

tick
echo "h3" > file.txt
git add file.txt
git commit -m "h3"

# Now create the first interesting branch
git checkout main

tick
git checkout -b feature1
echo "i1" > file.txt
git add file.txt
git commit -m "i1"

tick
echo "i3" > file.txt
git add file.txt
git commit -m "i3"

# Now create the second interesting branch
git checkout main

tick
git checkout -b feature2
echo "i2" > file.txt
git add file.txt
git commit -m "i2"

tick
echo "i4" > file.txt
git add file.txt
git commit -m "i4"

# Write out commit graph for faster lookups during test
git commit-graph write --no-progress --reachable
git repack -adq
