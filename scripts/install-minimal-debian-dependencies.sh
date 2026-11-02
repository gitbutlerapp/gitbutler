#!/bin/bash

set -eu -o pipefail

apt_cache_archives_dir="${1:-}"
apt_options=(-o Acquire::Retries=3 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30)
if [[ -n "${apt_cache_archives_dir}" ]]; then
    mkdir -p "${apt_cache_archives_dir}/partial"
    apt_options+=(-o "dir::cache::archives=${apt_cache_archives_dir}")
fi

# A mirror can stall a download without dropping the connection, which apt's own
# timeouts don't catch, so each attempt is cut off and retried. Without --error-on=any,
# a failed index download is only a warning and isn't retried.
for attempt in 1 2 3; do
    if timeout 120 apt-get update --error-on=any "${apt_options[@]}"; then
        break
    fi
    if [[ "${attempt}" == 3 ]]; then
        exit 1
    fi
done

# Install the dependencies needed to build the but-server and the CLI.
timeout 300 apt-get install -y --no-install-recommends "${apt_options[@]}" libdbus-1-dev pkg-config
