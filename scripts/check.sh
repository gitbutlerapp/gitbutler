#!/bin/bash

echo ------ cargo check
cargo c --all-features

echo ------ cargo clippy
cargo clippy --all-features
cargo clippy --tests

echo ------ cargo test
cargo t

echo ------ cargo fmt
cargo fmt --check

echo ------ cargo sort
cargo sort -c -w
