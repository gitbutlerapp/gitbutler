#!/usr/bin/env bash

### Description
# Minimal repository for watch-plan dynamic watch E2E tests.

set -eu -o pipefail

git init

# These should already exist after `git init`, but keep the fixture deterministic.
mkdir -p .git/logs .git/refs/heads

