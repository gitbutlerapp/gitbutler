#!/usr/bin/env bash
set -eu -o pipefail

git init -b main

printf "a" >foo.txt
printf "a" >bar.txt
git add foo.txt bar.txt
git commit -m "base"

git checkout -b target-foo
printf "b" >foo.txt
printf "a" >bar.txt
git add foo.txt bar.txt
git commit -m "target foo"
git tag target-foo

git checkout main
git checkout -b incoming-foo
printf "c" >foo.txt
printf "a" >bar.txt
git add foo.txt bar.txt
git commit -m "incoming foo"
git tag incoming-foo

git checkout main
git checkout -b target-bar
printf "a" >foo.txt
printf "b" >bar.txt
git add foo.txt bar.txt
git commit -m "target bar"
git tag target-bar

git checkout main
git checkout -b incoming-bar
printf "a" >foo.txt
printf "c" >bar.txt
git add foo.txt bar.txt
git commit -m "incoming bar"
git tag incoming-bar
