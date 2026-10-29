#!/usr/bin/env bash
set -eu -o pipefail

git init -b master
printf "content1" >file1.txt
git add file1.txt
git commit -m "initial commit"
