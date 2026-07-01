#!/usr/bin/env bash
set -eu -o pipefail

git init -b master
printf "actual content" >soon-fifo
git add soon-fifo
git commit -m "initial commit"
