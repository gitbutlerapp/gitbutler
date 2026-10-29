#!/usr/bin/env bash
set -eu -o pipefail

git init -b master
printf "tracked content" >deleted.txt
git add deleted.txt
git commit -m "Track deleted fixture file"
