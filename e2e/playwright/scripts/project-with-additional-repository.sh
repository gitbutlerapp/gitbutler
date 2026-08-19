#!/bin/bash

set -e

script_dir="$(cd "$(dirname "$0")" && pwd)"

bash "$script_dir/project-with-remote-branches.sh"
bash "$script_dir/project-with-named-branch.sh" additional-repository additional-branch
