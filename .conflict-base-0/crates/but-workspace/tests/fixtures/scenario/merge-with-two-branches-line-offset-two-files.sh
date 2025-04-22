#!/usr/bin/env bash

### Description
# Two branches on top of a common base and two files (one executable),
# one commit each, A puts 10 lines on top, B puts 10 lines to the bottom, no overlap.
set -eu -o pipefail

git init
seq 10 20 >file
seq 45 65 >other-file
git add . && git commit -m init

git branch B
git checkout -b A
seq 35 65 >other-file
seq 20 >file && git commit -am "add 10 to the beginning"

git checkout B
seq 45 75 >other-file
seq 10 30 >file && git commit -am "add 10 to the end"

git checkout -b merge
git merge A
