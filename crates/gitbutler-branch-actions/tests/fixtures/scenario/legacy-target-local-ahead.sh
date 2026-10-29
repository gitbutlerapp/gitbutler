#!/usr/bin/env bash
set -eu -o pipefail
source "${BASH_SOURCE[0]%/*}/local-target-different-upstream-name.sh"
echo local >local-only
git add local-only
git commit -m local-only
