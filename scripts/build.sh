#!/bin/bash

set -o errexit
set -o pipefail
set -o nounset

PWD="$(dirname $(readlink -f -- $0))"

echo $PWD
