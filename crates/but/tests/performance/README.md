# `but` CLI performance tests

End-to-end benchmarks for complete `but` subprocesses against GitButler-sized history.
Hyperfine owns timing and statistics; shell scripts own deterministic fixture setup.

## Prerequisites

- POSIX shell
- Git
- [Hyperfine](https://github.com/sharkdp/hyperfine)
- Rust/Cargo, unless `BUT_BIN` points to an existing optimized binary

## Running

From repository root:

```sh
# Run all scenarios
./crates/but/tests/performance/run.sh

# Run one scenario
./crates/but/tests/performance/run.sh status-many-uncommitted-changes
```

Use existing binary or shorten development run:

```sh
BUT_BIN=/absolute/path/to/but \
PERF_WARMUP=0 \
PERF_RUNS=2 \
./crates/but/tests/performance/run.sh squash-10-committed-hunks
```

Defaults are three warmups and at least twenty measured runs. `run.sh` builds `but`
with Cargo's optimized `bench` profile when `BUT_BIN` is unset. Always name scenario
explicitly while developing so unrelated scenarios do not run.

## Timing boundary

For every Hyperfine warmup and measured run:

1. Scenario `setup.sh` runs through `hyperfine --prepare`. It is not timed.
2. Scenario `test.sh` is timed in full.

Compilation, repository creation, GitButler setup, scenario commits, and selector
discovery are excluded. Process startup, CLI parsing, repository loading, operation,
and output generation are included. Scenario output is normally redirected to
`/dev/null` for consistency.

A smoke setup and test run happens before Hyperfine starts. This catches broken
fixtures and selectors without adding failed sample.

## Repository fixture and isolation

`lib.sh` pins GitButler history to full commit ID. `run.sh` fetches only that commit,
its trees, and reachable ancestry into immutable session-level bare repository. It
does not copy source worktree state, Git configuration, hooks, GitButler metadata,
local refs, or tags.

Each sample creates:

```text
immutable session fixture
        ↓ shared historical objects
per-sample bare origin
        ↓ shared historical objects
per-sample GitButler workspace
```

Historical Git objects are content-addressed and immutable. Mutable state is not
shared: each sample owns its remote refs, repository refs, index, worktree, new
objects, project database, oplog, app-data, config, cache, logs, temporary files,
and home directory. Pushes can mutate only per-sample bare remote.

Scenario scripts may mutate anything under `$PERF_RUN_ROOT`. They must not write to
`$PERF_FIXTURE_REPO` or `$PERF_SOURCE_REPO`.

### Shared clones

Fixtures use `git clone --shared` to avoid copying full GitButler history for every
sample. Scenarios may freely modify files, commits, branches, and their per-sample
remote, but must not modify shared fixture, alter Git object alternates, or run
object-pruning maintenance. Benchmarks that need to mutate object storage require a
different fixture strategy.

## Configuration isolation

Runner rebuilds benchmark environment from allowlist. Among other controls:

- `HOME` and `E2E_TEST_APP_DATA_DIR` point inside per-sample directory.
- System and global Git configuration are disabled.
- Author, committer, dates, locale, timezone, diff context, and change ID are fixed.
- Signing and interactive prompting are disabled.
- Telemetry, update checks, agent notices, pagers, and background tasks are disabled.
- Source repository configuration and hooks are not copied.

Repository-owned `.gitattributes`, `.gitignore`, and source contents remain part of
fixture because they represent workload at pinned commit.

## Adding scenario

Each scenario directory contains exactly two executable POSIX shell scripts:

```text
crates/but/tests/performance/scenarios/<scenario-name>/
├── setup.sh  # restore complete pre-operation state; not timed
└── test.sh   # execute one measured operation
```

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

Pass optional second argument to use ancestor as workspace target while retaining all
history from session fixture:

```sh
perf_create_gitbutler_workspace "$PERF_REPO" "$target_commit"
```

This is useful for replaying real historical change. Select commit reachable from
pinned fixture and use its parent as target:

```sh
REAL_COMMIT=<full-oid>
TARGET_COMMIT=$(
    "$GIT_BIN" --git-dir="$PERF_FIXTURE_REPO" rev-parse "$REAL_COMMIT^"
)

perf_reset_run_root
perf_create_gitbutler_workspace "$PERF_REPO" "$TARGET_COMMIT"
perf_git branch performance-branch "$REAL_COMMIT"
perf_but apply performance-branch >/dev/null
applied_commit=$(perf_git rev-parse refs/heads/performance-branch)
[ "$applied_commit" = "$REAL_COMMIT" ] ||
    perf_die "applying real change unexpectedly rewrote commit: $applied_commit"
perf_but uncommit "$applied_commit" >/dev/null
```

Prefer real GitButler commits and representative repository states over tiny synthetic
data. Validate setup assumptions outside timed script, such as expected path count,
hunk count, or graph shape. Keep checks tolerant only where Git representation
legitimately varies.

Useful untimed wrappers:

```sh
perf_git status --short
perf_but status
```

### Sharing setup data with test

Hyperfine launches prepare and measured scripts as separate processes, so exported
shell variables do not carry over. Setup writes scalar state atomically to:

```text
$PERF_RUN_ROOT/scenario.env
```

Use shared helpers:

```sh
# setup.sh
perf_state_begin
perf_state_set TARGET_COMMIT "$target_commit"
perf_state_set SOURCE_ID "$source_id"
perf_state_commit

# test.sh
perf_use_run_environment
perf_state_load
: "${TARGET_COMMIT:?missing TARGET_COMMIT}"
: "${SOURCE_ID:?missing SOURCE_ID}"
```

State names must match `[A-Z_][A-Z0-9_]*`. Values are POSIX-quoted. Use state for
OIDs, selectors, paths, names, and numbers. Put multiline or binary data in file under
`$PERF_RUN_ROOT` and pass its path through state.

Loading small state file occurs inside timed script. Do not run discovery commands
such as `but status` or `but diff` from `test.sh`.

### `test.sh`

Execute one measured `but` operation through `perf_exec_but`:

```sh
perf_use_run_environment
perf_state_load

perf_exec_but squash "$SOURCE_ID" --target "$TARGET_COMMIT" --use-target-message
```

`perf_exec_but` replaces script process with configured `but` binary. It sends output
to `/dev/null` normally and preserves it when `PERF_SHOW_OUTPUT=1`. Do not add
scenario-local output redirection. Keep `test.sh` limited to loading prepared state,
validating required values, and executing operation under study.

Add scenario and workload summary to [Included scenarios](#included-scenarios).

## Debugging

### Quick smoke run

Use one run and no warmup while iterating:

```sh
PERF_WARMUP=0 PERF_RUNS=1 \
./crates/but/tests/performance/run.sh <scenario-name>
```

One run validates behavior only; it is not meaningful performance measurement.

### Show `but` output

```sh
PERF_SHOW_OUTPUT=1 PERF_WARMUP=0 PERF_RUNS=1 \
./crates/but/tests/performance/run.sh <scenario-name>
```

This shows output from untimed smoke run and measured run. Runner also passes
Hyperfine's `--show-output`. Terminal rendering can dominate verbose commands, so use
this for debugging and benchmark normally without `PERF_SHOW_OUTPUT`. Results with and
without output display should not be compared directly.

### Separate setup failure from measured failure

Runner performs untimed smoke cycle before Hyperfine:

1. `setup.sh`
2. `test.sh`

Failure before `Benchmarking ...` is smoke/setup failure. With
`PERF_SHOW_OUTPUT=1`, inspect command output. Add temporary diagnostics to `setup.sh`
rather than adding discovery work to timed `test.sh`; remove diagnostics after fixing
scenario.

Typical setup diagnostics:

```sh
perf_but status >&2
perf_git log --oneline --decorate -10 >&2
perf_git status --porcelain=v1 >&2
cat "$PERF_RUN_ROOT/scenario.env" >&2
```

To test particular executable, set absolute path:

```sh
BUT_BIN=/absolute/path/to/but PERF_WARMUP=0 PERF_RUNS=1 \
./crates/but/tests/performance/run.sh <scenario-name>
```

## Validation

Run shell validation and short benchmark for changed scenario:

```sh
sh -n crates/but/tests/performance/*.sh \
      crates/but/tests/performance/scenarios/*/*.sh

shellcheck crates/but/tests/performance/*.sh \
           crates/but/tests/performance/scenarios/*/*.sh

PERF_WARMUP=0 PERF_RUNS=1 \
./crates/but/tests/performance/run.sh <scenario-name>
```

If ShellCheck is unavailable, report that explicitly. Check executable bits and update
[Included scenarios](#included-scenarios).

## Included scenarios

- `squash-10-committed-hunks`: squash ten committed hunks from one file into previous commit.
- `status-many-uncommitted-changes`: time `but status` after uncommitting real GitButler commit `702092d61c92c8d2093fc853b038c6f26b28207c`, which replaced insta snapshots with snapbox and changes 92 files, with 40,185 insertions and 30,208 deletions.

## Benchmark discipline

- Build once; never time `cargo run`.
- Restore state before every sample.
- Time only operation under study.
- Use fixed arguments and deterministic setup.
- Redirect output by default through `perf_exec_but`.
- Avoid network operations unless scenario explicitly benchmarks network.
- Use enough runs for final comparison; one run is only smoke validation.
- Do not treat shared CI wall time as strict regression signal.

## Interpreting results

Fixture restoration checks out many files and therefore warms filesystem caches.
Every timed operation still starts fresh process against fresh logical state. These are
warm-cache, fresh-process benchmarks—not cold-disk benchmarks.

Run comparisons on same idle machine under same power and thermal conditions. Shared
CI runners are generally too noisy for strict regression gates. Retain Hyperfine JSON
when historical comparisons are needed.
