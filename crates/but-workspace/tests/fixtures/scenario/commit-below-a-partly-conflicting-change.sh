#!/usr/bin/env bash

# Scenario: an uncommitted change rewrites line 2, which the stack's top commit wrote, and
# line 9, which no commit touched.
#
# Stack layout:
#   base (main) -> first (creates test.txt) -> second (line 2 to "two b")
#   Uncommitted: line 2 to "two c", line 9 to "nine c"
#
# Committing it below "second" takes line 9 and leaves out line 2.

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

seq 1 10 | sed 's/^2$/two/; s/^9$/nine/' > test.txt
git add test.txt
git commit -m "first"

sed -i.bak 's/^two$/two b/' test.txt && rm test.txt.bak
git commit -am "second"

sed -i.bak 's/^two b$/two c/; s/^nine$/nine c/' test.txt && rm test.txt.bak
