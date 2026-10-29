#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
commit-file file.txt one
git branch feature
git remote add origin ../origin
git update-ref refs/remotes/origin/main HEAD
echo two >file.txt
git commit -am two
