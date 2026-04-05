# Check all rust code - formatting, cargo check, and clippy.
.PHONY: all
all: fmt fmt-check check clippy

# Perform any automated formatting or clippy fixes that are available.
.PHONY: fix
fix: clippy-fix fmt

# Smaller - sub-checks

# Format: code to match rustfmt configuration.
.PHONY: fmt
fmt:
	cargo fmt

# Lint: code to ensure it matches our rustfmt configuration.
.PHONY: fmt-check
fmt-check:
	cargo fmt --check --all

# Lint: code to ensure there are no compiler errors or warnings.
.PHONY: check
check:
	cargo check --workspace --all-targets

# Lint: code to ensure rust best practices are observed.
.PHONY: clippy
clippy:
	cargo clippy --workspace --all-targets -- -D warnings

# An alias for nextest
.PHONY: test
test: nextest

# CI executes this to build a version of `but` without any legacy code.
.PHONY: check-modern-but
check-modern-but:
	cargo check -p but --all-targets --no-default-features

# CI executes this to test a version of `but` without any legacy code.
.PHONY: test-modern-but
test-modern-but:
	cargo test -p but --no-default-features

# Run all tests in the entire workspace and show all failures in the end.
.PHONY: nextest
nextest:
	cargo nextest run --workspace --no-fail-fast

# Format: clippy issues --allow-dirty allows the fix to be run even with a dirty
# working tree which can technically cause code to break, but I've never seen it
# happen in practice.
.PHONY: clippy-fix
clippy-fix:
	cargo clippy --workspace --all-targets --fix --allow-dirty

# Test: compile-time coverage for `but-api-macros` across relevant feature combinations.
.PHONY: test-but-api-macros
test-but-api-macros:
	cargo test -p but-api-macros-tests
	cargo test -p but-api-macros-tests --features legacy
	cargo test -p but-api-macros-tests --features tauri
	cargo test -p but-api-macros-tests --features napi
