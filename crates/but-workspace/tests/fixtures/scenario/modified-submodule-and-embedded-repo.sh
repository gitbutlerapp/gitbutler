#!/usr/bin/env bash

### Description
# A repository with a modified submodule and a modified embedded repository.
set -eu -o pipefail

git init embedded-repository
(cd embedded-repository
  echo content >file && git add . && git commit -m "init"
)

git init
git submodule add ./embedded-repository submodule
git add . && git commit -m "init"

(cd embedded-repository
  echo change >>file &&  git commit -am "change in embedded"
)

(cd submodule
  git pull --ff-only
  echo submodule-change >>file && git add file
  touch untracked
)


