#!/usr/bin/env bash

### Description
# A single branch with two commits. The first commit has 10 lines, the second adds
# another 4 lines to the top of the file.
# Large numbers are used make fuzzy-patch application harder.
set -eu -o pipefail

git init
seq 10 >file
git add . && git commit -m init && git tag first-commit

{ seq 4; seq 10; } >file && git commit -am "insert 5 lines to the top" && git branch feat1

git commit -m $'GitButler Workspace Commit\n\njust a fake - only the subject matters right now' --allow-empty
