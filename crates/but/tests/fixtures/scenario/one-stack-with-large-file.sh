#!/usr/bin/env bash
#
### General Description
#
# A single-stack workspace with a single relatively large file that can be used to (among other things) create independent diff hunks.

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen
commit-file initial-commit-file.txt
setup_target_to_match_main

git checkout -b large-file
# simulating `seq 100 > large_file.txt`
for i in {1..100}; do
  echo "$i" >> large_file.txt
done
git add large_file.txt && git commit -m 'Add large file'

create_workspace_commit_once large-file
