#!/bin/bash

set -eu -o pipefail

git init

seq 50 60 >file && git add . && git commit -m "base" && git tag base
git branch B

git checkout -b A
{ seq 10; seq 50 60; } >file && git add . && git commit -m "A: 10 lines on top"
git branch with-inner-merge

git checkout B
seq 10 >new-file && git add . && git commit -m "C: new file with 10 lines"

git checkout with-inner-merge && git merge --no-ff B
echo seq 10 >'added-after-with-inner-merge' && git add . && git commit -m "on top of inner merge"
