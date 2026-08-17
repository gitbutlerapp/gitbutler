#!/bin/bash

repository_name="${1:-onboarding-repository}"
branch_name="${2:-onboarding-test}"

git init -b "$branch_name" --object-format=sha1 "$repository_name"
echo "# Onboarding repository" > "$repository_name/README.md"
git -C "$repository_name" add README.md
git -C "$repository_name" commit -m "Initial commit"
