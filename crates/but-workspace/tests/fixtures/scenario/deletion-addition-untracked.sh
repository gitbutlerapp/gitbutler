#!/usr/bin/env bash

### Description
# Provide a deleted file, an added one, and an untracked one.
set -eu -o pipefail

git init
echo content >to-be-deleted
echo content2 >to-be-deleted-in-index
git add . && git commit -m "init"

echo new >added-to-index && git add added-to-index
echo untracked >untracked
rm to-be-deleted
git rm to-be-deleted-in-index

