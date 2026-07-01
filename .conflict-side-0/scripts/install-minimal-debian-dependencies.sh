#!/bin/bash

set -eu -o pipefail

apt_cache_archives_dir="${1:-}"
apt_options=()
if [[ -n "${apt_cache_archives_dir}" ]]; then
    mkdir -p "${apt_cache_archives_dir}/partial"
    apt_options=(-o "dir::cache::archives=${apt_cache_archives_dir}")
fi

# Install the dependencies needed to build the but-server and the CLI.
apt-get update
apt-get install -y --no-install-recommends "${apt_options[@]}" libdbus-1-dev pkg-config
