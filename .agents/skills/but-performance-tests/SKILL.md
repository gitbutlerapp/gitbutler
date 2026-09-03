---
name: but-performance-tests
description: Use when creating, changing, running, or debugging shell-based `but` CLI performance scenarios under `crates/but/tests/performance`, including Hyperfine options, fixture setup, setup-to-test state, and output inspection.
---

# `but` CLI performance tests

Read `crates/but/tests/performance/README.md` and nearby scenarios before making changes.
Keep benchmark changes inside `crates/but/tests/performance/`.

## Framework contract

Each scenario lives at:

```text
crates/but/tests/performance/scenarios/<scenario-name>/
├── setup.sh
└── test.sh
```

Both files must be executable POSIX shell scripts.

- `setup.sh` runs before every Hyperfine warmup and measured sample through `--prepare`; it is not timed.
- `test.sh` is timed in full.
- `run.sh` creates an immutable pinned GitButler history fixture once per session.
- Every sample gets its own bare remote, clone, worktree, GitButler app-data, config, cache, home, database, and oplog.
- Scenario may mutate anything below `$PERF_RUN_ROOT`; never mutate `$PERF_FIXTURE_REPO` or `$PERF_SOURCE_REPO`.
- Keep compilation, fixture creation, setup commands, and ID discovery out of `test.sh`.

## Create scenario

Start both scripts with:

```sh
#!/bin/sh
set -eu

: "${PERF_ROOT:?PERF_ROOT is not set}"
# shellcheck disable=SC1091
. "$PERF_ROOT/lib.sh"
```

### `setup.sh`

Reset sample and create complete GitButler workspace:

```sh
perf_reset_run_root
perf_create_gitbutler_workspace "$PERF_REPO"
```

To recreate a real historical change, select commit reachable from pinned fixture and use its parent as target:

```sh
REAL_COMMIT=<full-oid>
TARGET_COMMIT=$(
    "$GIT_BIN" --git-dir="$PERF_FIXTURE_REPO" rev-parse "$REAL_COMMIT^"
)

perf_reset_run_root
perf_create_gitbutler_workspace "$PERF_REPO" "$TARGET_COMMIT"
perf_git branch performance-branch "$REAL_COMMIT"
perf_but apply performance-branch >/dev/null
perf_but uncommit "$REAL_COMMIT" >/dev/null
```

Prefer real GitButler commits and representative repository states over tiny synthetic data. Validate setup assumptions outside timed script, such as expected path count, hunk count, or graph shape. Keep checks tolerant only where Git representation legitimately varies.

Useful untimed wrappers:

```sh
perf_git status --short
perf_but status
```

### Share setup data with test

Exports from `setup.sh` do not survive because Hyperfine launches separate processes. Write scalar state atomically:

```sh
perf_state_begin
perf_state_set TARGET_COMMIT "$target_commit"
perf_state_set SOURCE_ID "$source_id"
perf_state_commit
```

Load and validate it in `test.sh`:

```sh
perf_use_run_environment
perf_state_load
: "${TARGET_COMMIT:?missing TARGET_COMMIT}"
: "${SOURCE_ID:?missing SOURCE_ID}"
```

State names must match `[A-Z_][A-Z0-9_]*`. Use state for OIDs, selectors, paths, names, and numbers. Store multiline/binary payloads in files below `$PERF_RUN_ROOT` and pass paths through state.

Do not discover IDs with `but status` or `but diff` in timed script.

### `test.sh`

Execute one measured `but` operation through `perf_exec_but`:

```sh
perf_use_run_environment
perf_state_load

perf_exec_but squash "$SOURCE_ID" --target "$TARGET_COMMIT" --use-target-message
```

`perf_exec_but` replaces script process with configured `but` binary. It sends output to `/dev/null` normally and preserves it when `PERF_SHOW_OUTPUT=1`. Do not add scenario-local output redirection.

## Run scenario

From repository root:

```sh
./crates/but/tests/performance/run.sh <scenario-name>
```

Quick iteration:

```sh
PERF_WARMUP=0 PERF_RUNS=1 \
./crates/but/tests/performance/run.sh <scenario-name>
```

Specific binary:

```sh
BUT_BIN=/absolute/path/to/but \
./crates/but/tests/performance/run.sh <scenario-name>
```

Runner builds optimized `but` automatically when `BUT_BIN` is unset. Always name scenario explicitly while developing so unrelated scenarios do not run.

## Debug scenario

### See `but` output

```sh
PERF_SHOW_OUTPUT=1 PERF_WARMUP=0 PERF_RUNS=1 \
./crates/but/tests/performance/run.sh <scenario-name>
```

This shows output from untimed smoke run and measured run. It also passes Hyperfine `--show-output`.

Output display changes measured conditions: terminal rendering can dominate verbose commands. Use it for debugging, then benchmark normally without `PERF_SHOW_OUTPUT`.

### Separate setup failure from measured failure

Runner performs untimed smoke cycle before Hyperfine:

1. `setup.sh`
2. `test.sh`

Failure before `Benchmarking ...` is smoke/setup failure. With `PERF_SHOW_OUTPUT=1`, inspect command output. Add temporary diagnostics to `setup.sh` rather than adding discovery work to `test.sh`; remove diagnostics after fixing scenario.

Typical checks:

```sh
perf_but status >&2
perf_git log --oneline --decorate -10 >&2
perf_git status --porcelain=v1 >&2
cat "$PERF_RUN_ROOT/scenario.env" >&2
```

## Validate changes

Run:

```sh
sh -n crates/but/tests/performance/*.sh \
      crates/but/tests/performance/scenarios/*/*.sh

shellcheck crates/but/tests/performance/*.sh \
           crates/but/tests/performance/scenarios/*/*.sh

PERF_WARMUP=0 PERF_RUNS=1 \
./crates/but/tests/performance/run.sh <scenario-name>
```

If ShellCheck is unavailable, report that explicitly. Check executable bits and update scenario list in `crates/but/tests/performance/README.md`.

## Benchmark discipline

- Build once; never time `cargo run`.
- Restore state before every sample.
- Time only operation under study.
- Use fixed arguments and deterministic setup.
- Redirect output by default through `perf_exec_but`.
- Avoid network operations unless scenario explicitly benchmarks network.
- Use enough runs for final comparison; one run is only smoke validation.
- Do not treat shared CI wall time as strict regression signal.
