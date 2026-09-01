#!/bin/bash

set -euo pipefail

bash "$(dirname "$0")/project-with-remote-branches.sh"
git -C local-clone checkout master
