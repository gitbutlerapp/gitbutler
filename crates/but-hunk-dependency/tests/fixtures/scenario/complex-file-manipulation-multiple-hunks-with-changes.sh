#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

init-repo-with-files-and-remote

make-complex-file-manipulation-multiple-hunks-on-segment-no-workspace
  echo "added at the top
1
aaaaa
aaaaa
3
update 4 again
5
aaaaa
9
update bottom
add another line
" > file

create_workspace_commit_once my_stack