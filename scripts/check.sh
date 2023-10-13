#!/bin/bash

cargo c --all-features
cargo clippy --all-features
# cargo clippy -- --cfg test
cargo t
cargo fmt --check
cargo sort -c -w
