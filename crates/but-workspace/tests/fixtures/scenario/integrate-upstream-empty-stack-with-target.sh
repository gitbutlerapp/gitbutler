#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
echo "Empty stack with an advanced target branch for upstream merge integration" >.git/description

commit M1
setup_target_to_match_main

git branch A

git checkout main
commit upstream
setup_remote_tracking main

git checkout -b gitbutler/workspace A
