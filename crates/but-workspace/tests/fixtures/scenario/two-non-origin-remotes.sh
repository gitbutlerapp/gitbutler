#!/usr/bin/env bash

### Description
# An empty repository with two remotes, neither of them `origin`, so there is
# no unambiguous default remote to infer a target from.

set -eu -o pipefail

git init
git remote add upstream https://example.com/upstream
git remote add fork https://example.com/fork
