#!/bin/bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
git commit --allow-empty -m "commit with gitbutler change-id"

add_change_id_to_given_commit 1 "$(git rev-parse HEAD)" >.git/refs/heads/main

git remote add origin "https://gerrithost/project"


