#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

init-repo-with-files-and-remote

make-complex-file-manipulation-in-two-segments-no-workspace
create_workspace_commit_once top-series