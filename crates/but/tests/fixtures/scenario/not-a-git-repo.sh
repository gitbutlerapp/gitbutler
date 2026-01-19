#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# Not a git repository - just an empty directory
# This tests that `but setup` shows an error when run outside a git repository
