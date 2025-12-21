#!/bin/bash

set -eu -o pipefail

git init

echo "a" > base-f && git add . && git commit -m "base" && git branch base

git checkout -b single-clean-parent
echo "clean" > clean-f && git add . && git commit -m "single-clean-parent"

git checkout -b single-clean-commit
echo "clean-commit" > clean-commit-f && git add . && git commit -m "single-clean-commit"

git checkout -b single-target
git reset --hard base
echo "target" > target-f && git add . && git commit -m "single-target"

git checkout -b single-conflicting-commit
git reset --hard single-clean-parent
echo "conflict" > target-f && git add . && git commit -m "single-conflicting-commit"

git checkout -b second-target
git reset --hard base
echo "target 2" > target-2-f && git add . && git commit -m "second-target"

git checkout -b second-conflicting-target
git reset --hard base
echo "target 2" > target-f && git add . && git commit -m "second-conflicting-target"

git checkout -b second-clean-parent
git reset --hard base
echo "clean 2" > clean-2-f && git add . && git commit -m "second-clean-parent"

git checkout -b merge-clean-commit
git merge single-clean-parent
echo "clean-commit" > clean-commit-f && git add . && git commit -m "merge-clean-commit" --amend

git checkout -b merge-conflicting-commit
rm clean-commit-f && echo "conflict" > target-f && git add . && git commit -m "merge-conflicting-commit" --amend

git checkout -b second-conflicting-parent
git reset --hard base
echo "conflict" > clean-f && git add . && git commit -m "second-conflicting-parent"

git checkout -b merge-clean-commit-conflicting-parents
set +e
git merge single-clean-parent --no-edit
set -e
echo "resolved" > clean-f
git add .
git commit -m "foo"
echo "clean-commit" > clean-commit-f && git add . && git commit -m "merge-clean-commit-conflicting-parents" --amend

git checkout -b base-conflicting
git reset --hard base
echo "conflict" > target-f && git add . && git commit -m "base-conflicting" --amend
