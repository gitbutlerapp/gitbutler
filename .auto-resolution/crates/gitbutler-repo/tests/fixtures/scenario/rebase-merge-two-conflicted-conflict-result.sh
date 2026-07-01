#!/usr/bin/env bash
set -eu -o pipefail

git init -b main

printf "a" >foo.txt
git add foo.txt
git commit -m "base"

git checkout -b target-one
printf "b" >foo.txt
git add foo.txt
git commit -m "target one"
git tag target-one

git checkout main
git checkout -b incoming-one
printf "c" >foo.txt
git add foo.txt
git commit -m "incoming one"
git tag incoming-one

git checkout main
git checkout -b target-two
printf "d" >foo.txt
git add foo.txt
git commit -m "target two"
git tag target-two

git checkout main
git checkout -b incoming-two
printf "f" >foo.txt
git add foo.txt
git commit -m "incoming two"
git tag incoming-two
