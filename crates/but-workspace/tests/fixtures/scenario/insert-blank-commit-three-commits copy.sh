#!/usr/bin/env bash

set -eu -o pipefail

echo "/remote/" > .gitignore
mkdir remote
pushd remote
git init
popd

git init

git remote add origin ./remote

git checkout -b one
echo "foo" > one.txt
git add .
git commit -m "commit one"

git checkout -b two
echo "foo" > two.txt
git add .
git commit -m "commit two"

git push -u origin two

git checkout -b three
echo "foo" > three.txt
git add .
git commit -m "commit three"
