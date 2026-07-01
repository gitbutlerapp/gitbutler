#!/usr/bin/env bash

### Description
# A minimal example of a stack that has multiple branches on it, one of which is ambiguous
# on the top.
set -eu -o pipefail

git init
touch file && git add . && git commit -m "1"
git branch A
echo change >file && git commit -am "2"
# These commits are ambiguous in terms of bucket-order
git branch B
git branch C
