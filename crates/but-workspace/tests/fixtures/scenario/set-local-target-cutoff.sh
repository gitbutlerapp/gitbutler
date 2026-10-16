#!/usr/bin/env bash
set -eu -o pipefail
source "${BASH_SOURCE[0]%/*}/branch-diff-local-target-ahead.sh"
git checkout main
