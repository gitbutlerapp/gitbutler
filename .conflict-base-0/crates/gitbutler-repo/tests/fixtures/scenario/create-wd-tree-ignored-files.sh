#!/usr/bin/env bash
set -eu -o pipefail

git init -b master
printf "content" >tracked
printf "*.ignored" >.gitignore
git add tracked .gitignore
git commit -m "initial commit"
