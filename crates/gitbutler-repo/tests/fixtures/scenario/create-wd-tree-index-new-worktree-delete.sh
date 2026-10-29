#!/usr/bin/env bash
set -eu -o pipefail

git init -b master
git commit --allow-empty -m "initial commit"
printf "content1" >file1.txt
git add file1.txt
rm file1.txt
