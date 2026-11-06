#!/usr/bin/env bash

# Scenario: an uncommitted change rewrites a line the stack's top commit wrote,
# so it builds on that commit.
#
# Stack layout:
#   base (main) -> first (creates test.txt) -> second (line 2 to "two b")
#   Uncommitted: line 2 to "two c"
#
# Amending it into "first" conflicts.

set -eu -o pipefail

echo "/remote/" > .gitignore
mkdir remote
pushd remote
git init --bare
popd

git init

git remote add origin ./remote

echo "base" > base.txt
git add .
git commit -m "base"
git branch -M main
git push -u origin main

git checkout -b stack-1

printf "one\ntwo\nthree\n" > test.txt
git add test.txt
git commit -m "first"

printf "one\ntwo b\nthree\n" > test.txt
git commit -am "second"

printf "one\ntwo c\nthree\n" > test.txt
