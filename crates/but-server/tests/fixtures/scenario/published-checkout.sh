#!/usr/bin/env bash

set -eu -o pipefail

git init
echo "a checkout to publish to a hosted server, on main" >.git/description

git config user.name GitButler
git config user.email gitbutler@example.com

echo "hello" >file
git add file
GIT_COMMITTER_DATE="1675176957 +0100" GIT_AUTHOR_DATE="1675176957 +0100" git commit -m "initial"
