#!/bin/bash

set -eu -o pipefail

git init

echo base >base && git add . && git commit -m "main: base"
echo main-1 >main-1 && git add . && git commit -m "main: tip"

# Build a truly disjoint history rooted at an orphan branch.
git checkout --orphan orphan

git rm -rf . >/dev/null 2>&1 || true
echo orphan-base >orphan-base && git add . && git commit -m "orphan: base"
echo orphan-tip >orphan-tip && git add . && git commit -m "orphan: tip"

git checkout main
