#!/usr/bin/env bash

set -eu -o pipefail

git init

git checkout -b main
echo "base" >file.txt
git add file.txt
git commit -m "base"

git checkout -b branch
echo "one" >file.txt
git add file.txt
git commit -m "write one"

echo "two" >file.txt
git add file.txt
git commit -m "write two"

echo "three" >file.txt
git add file.txt
git commit -m "write three"
