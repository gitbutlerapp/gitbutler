#!/usr/bin/env bash
set -eu -o pipefail
source "${BASH_SOURCE[0]%/*}/legacy-target-local-behind.sh"
echo local >local-only
git add local-only
git commit -m local-only
