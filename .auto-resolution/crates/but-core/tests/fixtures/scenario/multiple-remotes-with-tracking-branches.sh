#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# A repository that has multiple remotes with different names.

git init normal-remote
(cd normal-remote 
    commit init-a
    git branch normal-remote
)

git init nested-remote
(cd nested-remote 
    commit init-b
    git branch in-nested-remote
)

git init nested-remote-b
(cd nested-remote-b
    commit init-c
    git branch in-nested-remote-b
)

git init
git remote add origin normal-remote
git remote add nested/remote nested-remote
git remote add nested/remote-b nested-remote-b

# NOTE: `git fetch` automatically creates remote HEAD refs (e.g.
# refs/remotes/origin/HEAD) since git 2.48. Tests relying on these
# refs require at least that version.
git fetch origin
git fetch nested/remote
git fetch nested/remote-b
