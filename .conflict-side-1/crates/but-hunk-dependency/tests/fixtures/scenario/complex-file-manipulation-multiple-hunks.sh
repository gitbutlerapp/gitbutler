#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

init-repo-with-files-and-remote

make-complex-file-manipulation-multiple-hunks-on-segment-no-workspace
create_workspace_commit_once my_stack