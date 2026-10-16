#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

# Files in the common base can be changed on either parallel branch. Leave
# enough context in `file` for edits at its ends to become separate hunks.
git-init-frozen
printf 'enough\nlines\nto\ncreate\nmultiple\nhunks\nwhen\nediting' >file
printf 'original second\n' >second
git add file second
git commit -m 'add files'
setup_target_to_match_main

git checkout -b A
commit-file A
create_workspace_commit_once A
