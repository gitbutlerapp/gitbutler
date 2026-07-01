#!/usr/bin/env bash
set -eu -o pipefail

git init -b main

printf "a" >foo.txt
git add foo.txt
git commit -m "base"

git checkout -b target
printf "b" >foo.txt
git add foo.txt
git commit -m "target"
git tag target

git checkout main
git checkout -b incoming
printf "c" >foo.txt
git add foo.txt
git commit -m "incoming"
git tag incoming
