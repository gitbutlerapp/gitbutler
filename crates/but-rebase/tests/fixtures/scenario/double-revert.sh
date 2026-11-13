#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init

tick
echo A >file.txt
git add file.txt
git commit -m A

tick
echo B >file.txt
git add file.txt
git commit -m B

tick
echo A >file.txt
git add file.txt
git commit -m "revert to A"
