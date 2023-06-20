#!/bin/bash

# Run pnpm format
pnpm format

# Change directory to src-tauri
cd src-tauri

# Run cargo fmt
cargo fmt

