#!/usr/bin/env bash
set -eu -o pipefail

git init -b master
mkdir dir1 dir2
printf "content1" >dir1/file1.txt
printf "content2" >dir2/file2.txt
git add dir1/file1.txt dir2/file2.txt
git commit -m "initial commit"
