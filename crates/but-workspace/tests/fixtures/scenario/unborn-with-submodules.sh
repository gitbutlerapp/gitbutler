#!/usr/bin/env bash

### Description
# A newly initialized git repository with with an added (but not committed) submodule.
set -eu -o pipefail

git init module
(cd module
  echo content >file && git add . && git commit -m "init"
)

git init
git submodule add ./module m1
