#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

init-repo-with-files-and-remote

make-complex-file-manipulation-in-two-segments-no-workspace
  echo "d
updated line 3
updated line 4
updated line 5
7
8
9
a
b
c" > file

  echo "1
b
c
updated d
e
f
" > file_2

create_workspace_commit_once top-series