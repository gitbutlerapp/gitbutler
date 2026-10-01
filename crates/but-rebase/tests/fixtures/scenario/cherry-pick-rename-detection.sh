#!/bin/bash

# Fixture for testing that workspace merges (TreeMergeMode::WithoutRenames) correctly
# surface delete-vs-modify conflicts instead of hiding them behind rename detection.
#
# Setup:
#   base has: file-a.txt, file-b.txt
#   stack-1: modifies file-b.txt (leaves file-a.txt unchanged)
#   stack-2-after: deletes file-a.txt AND file-b.txt, adds file-combined.txt
#
# file-combined.txt has the same content as file-b.txt, which causes exact
# rename detection to match file-b → file-combined. That hides the
# delete-vs-modify conflict on file-b.txt and moves stack-1's modification to
# an unrelated path.

set -eu -o pipefail

git init

# Base commit with two files. file-b's content will be reused byte-for-byte by
# stack-2-after, while file-a is an unrelated clean deletion.
cat > file-a.txt << 'CONTENT'
This is file A with unique content.
It has nothing in common with the combined file.
Alpha beta gamma delta.
Epsilon zeta eta theta.
CONTENT

cat > file-b.txt << 'CONTENT'
This is the test runner configuration.
It sets up the playwright test environment.
Line 3 of the config.
Line 4 of the config.
Line 5 of the config.
CONTENT

git add . && git commit -m "base"
git branch base

# stack-1: modifies file-b.txt, leaves file-a.txt unchanged
git checkout -b stack-1
cat > file-b.txt << 'CONTENT'
This is the test runner configuration.
It sets up the playwright test environment.
Line 3 of the config - updated by stack 1.
Line 4 of the config.
Line 5 of the config.
CONTENT
git add . && git commit -m "stack-1: modify file-b"

# stack-2-before: a benign change unrelated to file-a or file-b
git checkout -b stack-2-before
git reset --hard base
echo "unrelated" > other-file.txt && git add . && git commit -m "stack-2-before: unrelated change"

# stack-2-after: deletes both files and independently creates another path with
# file-b's exact content.
git checkout -b stack-2-after
git reset --hard base
cp file-b.txt file-combined.txt
rm file-a.txt file-b.txt
git add . && git commit -m "stack-2-after: combine files"

# Create the workspace commit: merge of stack-1 and stack-2-before
git checkout -b workspace-before
git reset --hard stack-1
git merge stack-2-before --no-edit -m "GitButler Workspace Commit"
