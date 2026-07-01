#!/usr/bin/env bash

# Scenario: A stack with two commits where the second was a partial commit,
# leaving uncommitted changes. Used to test amending those uncommitted changes
# back into the first commit.
#
# Stack layout:
#   base (main) -> save 1 (creates test.txt) -> partial 1 (adds line 1.1)
#   Uncommitted: adds line 1.2 between line 1.1 and line 2
#
# After amending line 1.2 into "save 1", the second commit should conflict
# and there should be no remaining uncommitted changes.

set -eu -o pipefail

echo "/remote/" > .gitignore
mkdir remote
pushd remote
git init --bare
popd

git init

git remote add origin ./remote

# Base commit on main
echo "base" > base.txt
git add .
git commit -m "base"
git branch -M main
git push -u origin main

# Create the stack branch
git checkout -b stack-1

# First commit: create test.txt with 3 lines
printf "line 1\nline 2\nline 3\n" > test.txt
git add test.txt
git commit -m "save 1"

# Second commit: partial commit adding only line 1.1
printf "line 1\nline 1.1\nline 2\nline 3\n" > test.txt
git add test.txt
git commit -m "partial 1"

# Now add the uncommitted change (line 1.2) that was left out of partial 1
printf "line 1\nline 1.1\nline 1.2\nline 2\nline 3\n" > test.txt
