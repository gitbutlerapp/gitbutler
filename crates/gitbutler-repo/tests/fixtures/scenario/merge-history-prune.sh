#!/bin/bash

set -eu -o pipefail

git init -b main

echo base >base-file
git add base-file
git commit -m "base"

git checkout -b A
echo A >a-file
git add a-file
git commit -m "A"
git branch C

git checkout -b B A
echo B >b-file
git add b-file
git commit -m "B"

git checkout A
git merge --no-ff B -m "merge B into A"

git checkout -b D C
echo D >d-file
git add d-file
git commit -m "D"

git checkout C
echo C >c-file
git add c-file
git commit -m "C"
git merge --no-ff D -m "merge D into C"

git checkout -b merged A
git merge --no-ff C -m "merge C into merged"
