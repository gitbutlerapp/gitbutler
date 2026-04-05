#!/usr/bin/env bash

set -eu -o pipefail

git init

git checkout -b one
echo "v1" > shared.txt
git add shared.txt
git commit -m "commit one"

git checkout -b two
echo "v2" > shared.txt
git add shared.txt
git commit -m "commit two"

git checkout -b three
echo "v3" > shared.txt
git add shared.txt
git commit -m "commit three"
