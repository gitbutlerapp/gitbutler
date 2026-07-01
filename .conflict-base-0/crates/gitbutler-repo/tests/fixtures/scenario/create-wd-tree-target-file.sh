#!/usr/bin/env bash
set -eu -o pipefail

git init -b master
printf "helloworld" >target
git add target
git commit -m "initial commit"
