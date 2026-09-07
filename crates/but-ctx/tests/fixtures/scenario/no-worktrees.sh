#!/bin/bash

set -eu -o pipefail

# A single commit and no linked worktrees, so adoption has nothing to archive.
git init
git commit --allow-empty -m M
