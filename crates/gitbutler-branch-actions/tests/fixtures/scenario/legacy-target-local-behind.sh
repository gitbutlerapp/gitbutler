#!/usr/bin/env bash
set -eu -o pipefail
source "${BASH_SOURCE[0]%/*}/local-target-upstream-ahead.sh"
git checkout main
git branch -D gitbutler/workspace
