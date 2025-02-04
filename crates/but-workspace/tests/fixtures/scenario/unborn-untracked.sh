#!/usr/bin/env bash

### Description
# A newly initialized git repository with a single untracked file.
set -eu -o pipefail

git init
echo content > not-yet-tracked
