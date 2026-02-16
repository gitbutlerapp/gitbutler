#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A repository with a remote configured but no remote/HEAD
# This tests that `but setup` guesses the default branch (main or master)
git-init-frozen
commit M

# Set up remote tracking for main but don't create HEAD
mkdir -p .git/refs/remotes/origin
cp .git/refs/heads/main .git/refs/remotes/origin/main

# Add remote config without HEAD
cat <<EOF >>.git/config
[remote "origin"]
	url = ./fake/local/path/which-is-fine-as-we-dont-fetch-or-push
	fetch = +refs/heads/*:refs/remotes/origin/*
EOF
