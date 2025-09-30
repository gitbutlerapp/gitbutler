#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a non-managed commit, along with two branches pointing to the same.
# This is an initial state where a workspace was newly created.
git init
commit A
git branch A
git branch B
git checkout -b gitbutler/workspace
