#!/usr/bin/env bash

set -eu -o pipefail

git init

echo base >file && git add . && git commit -m "base" && git tag base

git checkout -b feature-parent
echo parent-one >>parent.txt && git add . && git commit -m "add parent.txt"
echo parent-two >>parent.txt && git add . && git commit -m "update parent.txt (1)"
echo parent-three >>parent.txt && git add . && git commit -m "update parent.txt (2)"

git checkout -b main-advanced base
echo main-one >>main.txt && git add . && git commit -m "update main.txt (1)"
git branch -f main
git update-ref refs/remotes/origin/main main

git checkout -b child-stack main
git merge --no-ff feature-parent -m "M: merge feature-parent"
git branch M

echo child-one >child-1.txt && git add . && git commit -m "C1: add child-1.txt"
git branch C1

echo other >other.txt && git add . && git commit -m "C2: add other.txt"
git branch C2
