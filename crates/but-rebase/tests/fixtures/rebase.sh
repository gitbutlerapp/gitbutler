#!/bin/bash

set -eu -o pipefail

git init four-commits
(cd four-commits
  echo "base" > base
  git add .
  git commit -m "base"

  echo "a" > a
  git add .
  git commit -m "a"

  echo "b" > b
  git add .
  git commit -m "b"

  echo "c" > c
  git add .
  git commit -m "c"
)
