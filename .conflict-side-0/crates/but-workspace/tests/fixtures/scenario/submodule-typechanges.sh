#!/usr/bin/env bash

### Description
# A repository with a file and a submodule, with the file turning into a submodule and the submodule turning into a file.
# Note also the lack of rename tracking across types.
set -eu -o pipefail

git init embedded-repository
(cd embedded-repository
  echo content >file && git add . && git commit -m "init"
)

git init
git submodule add ./embedded-repository submodule
echo content >file
git add . && git commit -m "init"

git mv file tmp
git mv submodule file
git mv tmp submodule
