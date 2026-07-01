#!/usr/bin/env bash
set -eu -o pipefail

git init -b master
printf "this tracked file becomes a directory" >soon-directory
git add soon-directory
git commit -m "initial commit"
