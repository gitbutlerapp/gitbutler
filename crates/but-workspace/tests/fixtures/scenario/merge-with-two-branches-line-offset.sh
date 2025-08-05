#!/usr/bin/env bash

### Description
# Two branches on top of a common base, one commit each, A puts 10 lines on top,
# B puts 10 lines to the bottom, no overlap.
set -eu -o pipefail

git init
seq 10 20 >file
git add . && git commit -m init

git branch B
git checkout -b A
seq 20 >file && git commit -am "add 10 to the beginning"

git checkout B
seq 10 30 >file && git commit -am "add 10 to the end"

git checkout -b merge
git merge A
