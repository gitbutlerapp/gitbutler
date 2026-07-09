#!/usr/bin/env bash

### Description
# A local repository with a single commit on `main`, pushed to an `origin` remote
# so `refs/remotes/origin/main` exists. No workspace, no target - the starting
# point for initializing a project.

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init --bare remote.git
git init
commit M
git remote add origin ./remote.git
# A second remote so tests can tell a preserved push remote apart from one
# defaulted to the target's remote.
git remote add fork ./remote.git
git push --quiet -u origin main
