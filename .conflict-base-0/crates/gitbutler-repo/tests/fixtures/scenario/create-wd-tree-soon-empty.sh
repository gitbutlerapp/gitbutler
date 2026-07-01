#!/usr/bin/env bash
set -eu -o pipefail

git init -b master
printf "content" >soon-empty
git add soon-empty
git commit -m "initial commit"
