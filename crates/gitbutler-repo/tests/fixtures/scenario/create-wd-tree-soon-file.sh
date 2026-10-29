#!/usr/bin/env bash
set -eu -o pipefail

git init -b master
mkdir soon-file
printf "this tracked is removed and the parent dir becomes a file" >soon-file/content
git add soon-file/content
git commit -m "initial commit"
