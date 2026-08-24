#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/pull-two-integrated-stacks.sh"

old_target=$(git rev-parse A~1)
git init --bare .git/upstream.git
git push .git/upstream.git refs/heads/main:refs/heads/main
git remote set-url origin .git/upstream.git
git config branch.main.remote origin
git config branch.main.merge refs/heads/main
git update-ref refs/heads/main "$old_target"
