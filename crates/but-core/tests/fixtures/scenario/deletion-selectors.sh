#!/usr/bin/env bash

set -eu -o pipefail

git init
printf 'first\nsecond\nthird\n' >file
git add file
git commit -m "three selected lines"
rm file