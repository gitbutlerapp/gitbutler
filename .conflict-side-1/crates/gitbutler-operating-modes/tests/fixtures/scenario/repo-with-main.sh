#!/usr/bin/env bash

set -eu -o pipefail

git init -b main
git config user.name test
git config user.email test@example.com

touch tracked-file
git add tracked-file
git commit -m "initial commit"
