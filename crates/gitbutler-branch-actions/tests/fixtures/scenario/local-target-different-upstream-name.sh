#!/usr/bin/env bash
set -eu -o pipefail
source "${BASH_SOURCE[0]%/*}/local-target-upstream-ahead.sh"

git checkout main
git branch -D gitbutler/workspace
git update-ref refs/remotes/origin/trunk main
git update-ref -d refs/remotes/origin/main
git config branch.main.merge refs/heads/trunk
