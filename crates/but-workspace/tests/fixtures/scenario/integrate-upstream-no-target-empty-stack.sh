#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
echo "Empty stack without a target ref for upstream integration" >.git/description

commit M1
git checkout -b gitbutler/workspace
git branch A
