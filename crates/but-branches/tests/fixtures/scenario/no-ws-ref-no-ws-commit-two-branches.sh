#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A situation where no workspace reference is available yet.
git init

commit A
setup_target_to_match_main

git branch A
git branch B

