#!/usr/bin/env bash
set -eu -o pipefail

git init -b master
printf "still small enough" >soon-too-big
git add soon-too-big
git commit -m "initial commit"
