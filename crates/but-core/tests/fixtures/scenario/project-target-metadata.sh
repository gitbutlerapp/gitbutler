#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

# Local main trails its differently named upstream. Other branches are untracked.
git init
git checkout -b main
commit base
git branch other
commit upstream
setup_remote_tracking main trunk
git reset --hard HEAD~1
git remote add origin https://example.com/upstream.git
git remote add fork https://example.com/fork.git
git config branch.main.remote origin
git config branch.main.merge refs/heads/trunk
