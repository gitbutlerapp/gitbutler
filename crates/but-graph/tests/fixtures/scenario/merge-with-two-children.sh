#!/bin/bash

set -eu -o pipefail

git init

echo base >file && git add . && git commit -m "base" && git tag base

git checkout -b P1
echo parent-one >>file && git add . && git commit -m "P1: first merge parent"

git checkout -b P2 base
echo parent-two >second-file && git add . && git commit -m "P2: second merge parent"

git checkout -b with-two-children P1
git merge --no-ff P2 -m "M: merge two parents"
git branch M

echo child-one >child-one && git add . && git commit -m "C1: first child"
git branch C1

git checkout -b C2 M
echo child-two >child-two && git add . && git commit -m "C2: second child"

git checkout with-two-children
git merge --no-ff C2 -m "tip"