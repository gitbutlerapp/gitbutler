#!/usr/bin/env bash

set -eu -o pipefail

git init

git config user.name user
git config user.email user@example.com
echo content > not-yet-tracked
