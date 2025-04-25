#!/usr/bin/env bash

### Description
# A repository with all file-types, including a submodule, which are all scheduled for deletion.
set -eu -o pipefail

git init embedded-repository
(cd embedded-repository
  echo content >file && git add . && git commit -m "init"
)

git init
git submodule add ./embedded-repository submodule
echo content >file-to-remain
echo exe >executable && chmod +x executable
ln -s file-to-remain link
git add . && git commit -m "init"

rm -Rf ./embedded-repository/ ./submodule/ executable link .gitmodules

