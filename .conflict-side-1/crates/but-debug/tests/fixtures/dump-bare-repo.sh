#!/usr/bin/env bash
set -eu -o pipefail

# Initial state for verifying that dumping a bare repository preserves the
# repository directory shape and extracts under a `*-dump.git` root.
git init --bare sample.git
