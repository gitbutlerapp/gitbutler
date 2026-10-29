#!/usr/bin/env bash

### Description
# A repository whose HEAD is unborn while remote-tracking branches already
# exist, as after `git init`, adding a remote, and fetching.
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init

tick
git checkout -b seed
commit "feature-1"
commit "feature-2"
turn_into_remote_branch seed feature

git symbolic-ref HEAD refs/heads/never-born
