#!/bin/bash

# Fixture for testing that workspace merges (TreeMergeMode::WithoutRenames) correctly
# surface delete-vs-modify conflicts instead of hiding them behind rename detection.
#
# Setup:
#   base has: file-a.txt, file-b.txt
#   stack-1: modifies file-b.txt (leaves file-a.txt unchanged)
#   stack-2-after: deletes file-a.txt AND file-b.txt, adds file-combined.txt
#
# file-combined.txt has content similar to file-b.txt, which would cause gix's
# rename detection to match file-b → file-combined as a rename. With rename
# detection enabled, this hides the delete-vs-modify conflict on file-b.txt
# and silently drops file-a.txt's deletion.

set -eu -o pipefail

git init

# Base commit with two files
# file-b has content that is very similar to what file-combined will have,
# so rename detection may heuristically match file-b → file-combined as a rename.
# file-a is different content, so it should just be a clean delete.
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

# stack-2-after: deletes both files and creates combined file
# file-combined has content very similar to file-b (rename detection target)
git checkout -b stack-2-after
git reset --hard base
rm file-a.txt file-b.txt
cat > file-combined.txt << 'CONTENT'
This is the test runner configuration.
It sets up the playwright test environment.
Line 3 of the config.
Line 4 of the config.
Line 5 of the config.
Additional combined content from both files.
CONTENT
git add . && git commit -m "stack-2-after: combine files"

# Create the workspace commit: merge of stack-1 and stack-2-before
git checkout -b workspace-before
git reset --hard stack-1
git merge stack-2-before --no-edit -m "GitButler Workspace Commit"
