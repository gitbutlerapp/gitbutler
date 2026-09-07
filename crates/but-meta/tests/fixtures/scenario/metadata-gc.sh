#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"
set -eu -o pipefail

git init
commit base
git branch before-target
commit target
git branch target
commit ahead
git branch ahead
git checkout target

# A direct ref with no object and a symbolic ref exercise distinct missing-head cases.
echo 30696678319e0fa3a20e54f22d47fc8cf1ceaade >.git/refs/heads/broken
git symbolic-ref refs/heads/symbolic refs/heads/target
