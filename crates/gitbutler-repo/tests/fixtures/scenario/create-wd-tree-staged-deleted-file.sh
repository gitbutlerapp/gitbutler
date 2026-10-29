#!/usr/bin/env bash
set -eu -o pipefail

git init -b master
git commit --allow-empty -m "initial commit"
printf "content1" >file1.txt
printf "content2" >file2.txt
printf "content2" >file3.txt
git add file3.txt
rm file3.txt
