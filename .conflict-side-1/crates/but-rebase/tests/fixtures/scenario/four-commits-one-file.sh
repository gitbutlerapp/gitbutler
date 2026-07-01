#!/bin/bash

set -eu -o pipefail

git init

echo "base" >a && git add . && git commit -m "base"
git update-ref refs/heads/base $(git rev-parse HEAD)
echo "a" >a && git add . && git commit -m "a"
git update-ref refs/heads/a $(git rev-parse HEAD)
echo "b" >a && git add . && git commit -m "b"
git update-ref refs/heads/b $(git rev-parse HEAD)
echo "c" >a && git add . && git commit -m "c"
git update-ref refs/heads/c $(git rev-parse HEAD)
