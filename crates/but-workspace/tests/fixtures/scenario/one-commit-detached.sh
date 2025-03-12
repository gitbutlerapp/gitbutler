#!/usr/bin/env bash

### Description
# A single commit from which the HEAD is detached.
set -eu -o pipefail

git init
touch file && git add . && git commit -m "init"
git checkout @
