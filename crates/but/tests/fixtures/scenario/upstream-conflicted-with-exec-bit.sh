#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# Branch A makes f executable and changes its content, and the target changes the same line.

git-init-frozen

printf 'line1\nbase\nline3\n' >f
git add f
git commit -m "base"
git update-ref refs/heads/base HEAD

git checkout -b A
printf 'line1\nchange-on-A\nline3\n' >f
chmod +x f
git add f
git commit -m "A-change"

git checkout main
printf 'line1\nchange-on-main\nline3\n' >f
git add f
git commit -m "main-change"
setup_target_to_match_main

git checkout A
create_workspace_commit_once A
