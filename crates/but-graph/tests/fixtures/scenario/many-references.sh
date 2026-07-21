#!/bin/bash

set -eu -o pipefail

git init

echo "base" >base && git add . && git commit -m "base"
echo "a" >a && git add . && git commit -m "a"
git branch X
git branch Y
git branch Z
echo "b" >b && git add . && git commit -m "b"
echo "c" >c && git add . && git commit -m "c"
