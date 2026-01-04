#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# Emulate a two-segment stack with names suitable for the respective vb.toml file.
set -eu -o pipefail

git init
commit M1
git branch confidence
commit M2

setup_target_to_match_main
create_workspace_commit_once main

# Switch back to main to simulate single-branch mode
git checkout main
