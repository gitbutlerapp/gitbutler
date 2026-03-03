#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

exec cargo run -p but --manifest-path "$repo_root/Cargo.toml" -- "$@"
