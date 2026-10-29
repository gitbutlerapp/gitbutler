#!/bin/bash

set -eu

branch="$1"
directory="${2:-local-clone}"

pushd "$directory"
"$BUT" push "$branch"
popd
