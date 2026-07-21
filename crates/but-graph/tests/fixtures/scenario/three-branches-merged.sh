#!/bin/bash

set -eu -o pipefail

git init

seq 50 60 >file && git add . && git commit -m "base" && git tag base
git branch B
git branch C

git checkout -b A
{ seq 10; seq 50 60; } >file && git add . && git commit -m "A: 10 lines on top"

git checkout B
{ seq 50 60; seq 61 70; } >file && git add . && git commit -m "B: 10 lines at the bottom"
{ seq 50 60; seq 61 80; } >file && git add . && git commit -m "B: another 10 lines at the bottom"

git checkout C
seq 10 >new-file && git add . && git commit -m "C: new file with 10 lines"
seq 20 >new-file && git add . && git commit -m "C: add 10 lines to new file"
seq 30 >new-file && git add . && git commit -m "C: add another 10 lines to new file"

git checkout main
git merge A B C
