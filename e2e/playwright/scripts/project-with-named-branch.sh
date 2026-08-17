#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

repository_name="${1:-onboarding-repository}"
branch_name="${2:-onboarding-test}"

git init -b "$branch_name" --object-format=sha1 "$repository_name"
echo "# Onboarding repository" > "$repository_name/README.md"
git -C "$repository_name" add README.md
git -C "$repository_name" commit -m "Initial commit"
