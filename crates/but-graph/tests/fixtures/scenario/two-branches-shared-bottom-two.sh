#!/bin/bash

set -eu -o pipefail

git init

echo "base" >base && git add . && git commit -m "base"
echo "shared" >shared && git add . && git commit -m "shared"

git branch right

echo "left-head" >left && git add . && git commit -m "left: head"
git branch left

git checkout right
echo "right-head" >right && git add . && git commit -m "right: head"

git checkout main
git merge right -m "merge right into main"