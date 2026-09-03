# `but` CLI performance tests

End-to-end benchmarks for complete `but` subprocesses against GitButler-sized history.
Hyperfine owns timing and statistics; shell scripts own deterministic fixture setup.

## Prerequisites

- POSIX shell
- Git
- [Hyperfine](https://github.com/sharkdp/hyperfine)
- Rust/Cargo, unless `BUT_BIN` points to an existing optimized binary

## Running

From any directory:

```sh
./crates/but/tests/performance/run.sh
```

Run selected scenarios:

```sh
./crates/but/tests/performance/run.sh status-many-uncommitted-changes
```

Use an existing binary or shorten a development run:

```sh
BUT_BIN=/absolute/path/to/but \
PERF_WARMUP=0 \
PERF_RUNS=2 \
./crates/but/tests/performance/run.sh squash-10-committed-hunks
```

Show command output during smoke test and every measured run:

```sh
PERF_SHOW_OUTPUT=1 PERF_WARMUP=0 PERF_RUNS=1 \
./crates/but/tests/performance/run.sh status-many-uncommitted-changes
```

Defaults are three warmups and at least twenty measured runs. `run.sh` builds `but`
with Cargo's optimized `bench` profile when `BUT_BIN` is unset.

## Timing boundary

For every Hyperfine warmup and measured run:

1. Scenario `setup.sh` runs through `hyperfine --prepare`. It is not timed.
2. Scenario `test.sh` is timed in full.

Compilation, repository creation, GitButler setup, scenario commits, and selector
discovery are excluded. Process startup, CLI parsing, repository loading, operation,
and output generation are included. Scenario output is normally redirected to
`/dev/null` for consistency. Set `PERF_SHOW_OUTPUT=1` to keep stdout/stderr and pass
Hyperfine's `--show-output`; terminal rendering then becomes part of measured cost and
results should not be compared directly with default runs.

A smoke setup and test run happens before Hyperfine starts. This catches broken
fixtures and selectors without adding a failed sample.

## Repository fixture and isolation

`lib.sh` pins GitButler history to a full commit ID. `run.sh` fetches only that
commit, its trees, and reachable ancestry into an immutable session-level bare
repository. It does not copy source worktree state, Git configuration, hooks,
GitButler metadata, local refs, or tags.

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
and home directory. Pushes can mutate only the per-sample bare remote.

Scenario scripts may mutate anything under `$PERF_RUN_ROOT`. They must not write
to `$PERF_FIXTURE_REPO` or `$PERF_SOURCE_REPO`.

## Configuration isolation

Runner rebuilds benchmark environment from an allowlist. Among other controls:

- `HOME` and `E2E_TEST_APP_DATA_DIR` point inside per-sample directory.
- system and global Git configuration are disabled.
- author, committer, dates, locale, timezone, diff context, and change ID are fixed.
- signing and interactive prompting are disabled.
- telemetry, update checks, agent notices, pagers, and background tasks are disabled.
- source repository configuration and hooks are not copied.

Repository-owned `.gitattributes`, `.gitignore`, and source contents remain part of
fixture because they represent workload at pinned commit.

## Sharing setup data with test

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
perf_state_set SOURCE_HUNK "$source_hunk"
perf_state_commit

# test.sh
perf_state_load
: "${TARGET_COMMIT:?missing TARGET_COMMIT}"
```

State names must match `[A-Z_][A-Z0-9_]*`. Values are POSIX-quoted. Use state for
OIDs, selectors, paths, and branch names. Put multiline or binary data in a file
under `$PERF_RUN_ROOT` and pass its path through state.

Loading small state file occurs inside timed script. Do not run discovery commands
such as `but status` or `but diff` from `test.sh`.

## Adding a scenario

Create directory under `scenarios/` containing exactly:

```text
setup.sh  # restore complete pre-operation state; not timed
test.sh   # execute one measured operation
```

Both files must be executable. Source shared library with:

```sh
: "${PERF_ROOT:?PERF_ROOT is not set}"
. "$PERF_ROOT/lib.sh"
```

`setup.sh` should call:

```sh
perf_reset_run_root
perf_create_gitbutler_workspace "$PERF_REPO"
```

Pass optional second argument to use an ancestor as workspace target while retaining
all history from session fixture:

```sh
perf_create_gitbutler_workspace "$PERF_REPO" "$target_commit"
```

Keep setup deterministic and validate assumptions before writing scenario state.
`test.sh` should load state if needed and finish with `exec "$BUT_BIN" ...`.

## Included scenarios

- `squash-10-committed-hunks`: squash ten committed hunks from one file into previous commit.
- `status-many-uncommitted-changes`: time `but status` after uncommitting real GitButler commit `702092d61c92c8d2093fc853b038c6f26b28207c`, which replaced insta snapshots with snapbox and changes 92 files, with 40,185 insertions and 30,208 deletions.

## Interpreting results

Fixture restoration checks out many files and therefore warms filesystem caches.
Every timed operation still starts a fresh process against fresh logical state.
These are warm-cache, fresh-process benchmarks—not cold-disk benchmarks.

Run comparisons on same idle machine under same power and thermal conditions.
Shared CI runners are generally too noisy for strict regression gates. Retain
Hyperfine JSON when historical comparisons are needed.
