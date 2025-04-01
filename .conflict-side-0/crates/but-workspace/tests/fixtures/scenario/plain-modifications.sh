#!/usr/bin/env bash

### Description
# An empty file that has 10 added lines, and a file with 10 lines that got emptied.
set -eu -o pipefail

git init
touch all-added
seq 10 >all-removed
cp all-removed all-modified
git add . && git commit -m "init"

seq 10 >all-added
rm all-removed && touch all-removed
seq 11 20 >all-modified