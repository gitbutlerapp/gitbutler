# `but` CLI performance tests

End-to-end benchmarks for complete `but` subprocesses against GitButler-sized history.
Hyperfine owns timing and statistics; shell scripts own deterministic fixture setup.

## Cheat sheet

Run from repository root. No scenario arguments means full suite.

```sh
# Full suite against latest nightly / stable release (Linux x86_64)
PERF_CHANNEL=nightly ./crates/but/tests/performance/run.sh
PERF_CHANNEL=release ./crates/but/tests/performance/run.sh

# Full suite against specific nightly version
PERF_CHANNEL=nightly PERF_VERSION=0.5.2189 ./crates/but/tests/performance/run.sh

# Full suite with local optimized build / existing binary
./crates/but/tests/performance/run.sh
BUT_BIN=/absolute/path/to/but ./crates/but/tests/performance/run.sh

# Quick single-scenario smoke run (not meaningful performance measurement)
PERF_CHANNEL=nightly PERF_WARMUP=0 PERF_RUNS=1 \
./crates/but/tests/performance/run.sh status-many-uncommitted-changes

# Show but output during smoke and measured runs for debugging
PERF_CHANNEL=nightly PERF_SHOW_OUTPUT=1 PERF_WARMUP=0 PERF_RUNS=1 \
./crates/but/tests/performance/run.sh status-many-uncommitted-changes

# Skip preliminary smoke test; keep Hyperfine setup and measured run
PERF_CHANNEL=nightly PERF_SKIP_SMOKE=1 PERF_WARMUP=0 PERF_RUNS=1 \
./crates/but/tests/performance/run.sh diff-many-committed-changes

# Save full-suite results as Hyperfine JSON
PERF_CHANNEL=nightly PERF_RESULTS_DIR="$PWD/target/performance-results" \
./crates/but/tests/performance/run.sh

# Full nightly suite with HTTP upload (supply suite token securely)
PERF_CHANNEL=nightly \
PERF_UPLOAD_URL=https://tests.but.dev \
PERF_UPLOAD_TOKEN="$TOKEN" \
./crates/but/tests/performance/run.sh
```

`PERF_SHOW_OUTPUT=1` disables normal output suppression. Use for debugging only:
terminal rendering affects timings, so do not compare with output-suppressed runs.

Uploads with supplied `BUT_BIN` also require `PERF_BINARY_COMMIT` (full binary commit
SHA). Downloaded binaries supply it automatically. Older releases may not support
all current scenarios.

## Running and results

Requires POSIX shell, Git, and [Hyperfine](https://github.com/sharkdp/hyperfine), plus:

- Rust/Cargo for local builds (not needed with `BUT_BIN` or `PERF_CHANNEL`).
- `curl` and `jq` for release downloads.
- `curl` and `jq` for HTTP uploads (not needed for local-only supplied/local builds).

Defaults: three warmups and at least twenty measured runs. Set `PERF_WARMUP`,
`PERF_MIN_RUNS`, or `PERF_RUNS` to adjust. Name one scenario while developing.
`PERF_SKIP_SMOKE=1` skips preliminary setup-and-operation smoke tests (default: `0`).
Only `0` and `1` are accepted. Hyperfine's per-run setup, warmups, and measured runs
are unaffected.

Binary selection: `PERF_CHANNEL` downloads nightly/release for Linux x86_64;
otherwise `BUT_BIN` selects existing binary, or runner builds optimized Cargo `bench`
profile. `PERF_VERSION` selects release's `version` (not `build_version`); omit for
latest. It only affects downloads.

`PERF_RESULTS_DIR` saves Hyperfine statistics and samples as `<scenario>.json`,
overwriting same-named files. Upload mode defaults results directory to
`target/performance-results`; local-only runs save nothing unless this variable is set.

## HTTP uploads and replay

Set `PERF_UPLOAD_URL` and `PERF_UPLOAD_TOKEN` to upload results; leave both unset
for local-only runs.

```sh
PERF_CHANNEL=nightly \
PERF_UPLOAD_URL=https://tests.but.dev PERF_UPLOAD_TOKEN="$TOKEN" \
./crates/but/tests/performance/run.sh
```

With `BUT_BIN`, also set `PERF_BINARY_COMMIT` to its full commit SHA. Optionally set
`PERF_MACHINE` and `PERF_CPU` to override hostname and CPU description.

Failed uploads print the saved payload path. Retry without rerunning benchmarks:

```sh
PERF_UPLOAD_URL=https://tests.but.dev PERF_UPLOAD_TOKEN="$TOKEN" \
./crates/but/tests/performance/upload.sh \
/path/to/results/uploads/<upload-id>/payload.json
```

Use the saved payload unchanged; replaying an accepted upload won't duplicate results.
For local testing, use a separate suite token and a loopback URL such as
`http://127.0.0.1:6979`; other URLs require HTTPS.

## Profiling one scenario

`profile.sh <backend> <scenario>` reuses benchmark scenarios, profiling only `but`:
compilation and setup stay outside capture.

```sh
./crates/but/tests/performance/profile.sh samply status-many-uncommitted-changes
./crates/but/tests/performance/profile.sh perf squash-10-committed-hunks
./crates/but/tests/performance/profile.sh flamegraph diff-many-uncommitted-changes
```

Requires Git, shell utilities, selected profiler, and Cargo unless supplying `BUT_BIN`.
Install samply/flamegraph with `cargo install --locked samply` / `cargo install --locked flamegraph`.
Linux perf/flamegraph also require kernel-compatible perf package.

| Backend | Platforms | View capture |
| --- | --- | --- |
| `samply` | Linux, macOS | `samply load <profile.json>` |
| `perf` | Linux | `perf report -i <perf.data>` |
| `flamegraph` | Linux, macOS with full Xcode/xctrace | Open `flamegraph.svg` in browser |

Configuration uses environment variables, not flags:

- `BUT_BIN`: existing binary; otherwise build optimized native binary with debug symbols.
- `PERF_PROFILE_OUTPUT_DIR`: new output directory; defaults beneath `target/performance-profiles/<scenario>/`.
- `PERF_SHOW_OUTPUT=1`: show command stdout.
- `DEVELOPER_DIR`: optional Xcode selection.

Captures, logs, metadata, and binary/debug symbols are retained; temporary fixtures
are removed. Keep captures at recorded path for symbol lookup. Benchmark timing,
upload, and download settings are unsupported.

**macOS:** prefer samply with locally built binary; full Xcode needed only for
flamegraph.

**Linux:** samply and perf may need `kernel.perf_event_paranoid=1` to allow
recording and a larger perf buffer allowance (`kernel.perf_event_mlock_kb=2048`,
in KiB). If recording fails due to permissions or buffer allocation, an administrator
can apply these machine-wide settings:

```sh
sudo sysctl -w kernel.perf_event_paranoid=1
sudo sysctl -w kernel.perf_event_mlock_kb=2048
```

These changes normally last until reboot and relax profiling restrictions for other
users too. Harness never changes these settings automatically. See
[samply](https://github.com/mstange/samply) and
[flamegraph](https://github.com/flamegraph-rs/flamegraph) docs for permissions and
symbolization troubleshooting (including lld/mold's `--no-rosegment` requirement).

## Included scenarios

- `diff-many-committed-changes`: time `but diff <commit>` on real GitButler formatting commit `c9d8e3a7ff59f2ddabed16a6fa1d66ea054f0215`, applied in a clean workspace, with 1,167 changed files, 21,636 insertions and 21,620 deletions.
- `diff-many-uncommitted-changes`: time `but diff` after uncommitting real GitButler commit `c9d8e3a7ff59f2ddabed16a6fa1d66ea054f0215`, which formatted the codebase and changes 1,167 files, with 21,636 insertions and 21,620 deletions.
- `squash-10-committed-hunks`: squash ten committed hunks from one file into previous commit.
- `status-large-uncommitted-file`: time `but status` with one untracked 400 MiB random binary file.
- `status-many-uncommitted-changes`: time `but status` after directly modifying 240 tracked Rust files with `core.autocrlf=input`.
- `status-many-uncommitted-changes-fragmented-odb`: same status workload with 200 additional small local packs. Reproduces [the null-ID object database rescan fixed by GitButler PR #15746](https://github.com/gitbutlerapp/gitbutler/pull/15746): with automatic text conversion enabled, each changed worktree file could trigger a guaranteed-miss null-ID lookup and rescan every pack.

## Measurement and fixture rules

Runner smoke-tests setup and operation before Hyperfine unless `PERF_SKIP_SMOKE=1`.
For every warmup and sample,
`setup.sh` runs untimed through `--prepare`; `test.sh` is timed in full, including
process startup and output generation. Downloads, compilation, fixture creation,
and selector discovery stay outside timing.

Each sample gets fresh workspace, bare remote, and isolated configuration, sharing
only immutable historical objects from pinned GitButler fixture. See [lib.sh](lib.sh)
for shared environment allowlist and fixture/setup details; [run.sh](run.sh) adds
benchmark-specific variables.

Scenario scripts may mutate anything under `$PERF_RUN_ROOT`, but must not write to
`$PERF_FIXTURE_REPO` or `$PERF_SOURCE_REPO`. Fixtures use `git clone --shared`: do not
alter object alternates or run object-pruning maintenance. Object-storage mutation
benchmarks need different fixture strategy. Repository-owned attributes and ignore
rules remain part of workload.

Fixture restoration warms filesystem caches: these are **warm-cache, fresh-process**
benchmarks, not cold-disk measurements. Compare on same idle machine under same power
and thermal conditions; shared CI timings are too noisy for strict regression gates.
One-run smoke checks validate behavior, not performance.

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

`perf_exec_but` replaces script process with configured `but` binary (or profiling
entrypoint's recorder when explicitly enabled for capture). It sends output
to `/dev/null` normally and preserves it when `PERF_SHOW_OUTPUT=1`. Do not add
scenario-local output redirection. Keep `test.sh` limited to loading prepared state,
validating required values, and executing operation under study.

Add scenario and workload summary to [Included scenarios](#included-scenarios).

## Debugging and validation

Use cheat sheet's single-run commands with `PERF_SHOW_OUTPUT=1`. Failure before
`Benchmarking ...` is smoke/setup failure. Add temporary diagnostics such as
`perf_but status >&2` or inspecting `$PERF_RUN_ROOT/scenario.env` to `setup.sh`, never
timed `test.sh`; remove them after fixing scenario.

Run shell validation and short benchmark for changed scenario:

```sh
for script in crates/but/tests/performance/*.sh \
              crates/but/tests/performance/scenarios/*/*.sh; do
    sh -n "$script" || exit
done

shellcheck crates/but/tests/performance/*.sh \
           crates/but/tests/performance/scenarios/*/*.sh

PERF_WARMUP=0 PERF_RUNS=1 \
./crates/but/tests/performance/run.sh <scenario-name>
```

For receiver integration, smoke-test a single scenario with
`PERF_WARMUP=0 PERF_RUNS=1`, a local receiver URL and benchmark-suite token, then replay
the retained payload and confirm duplicate acceptance. Smoke timings aren't regressions.

If ShellCheck is unavailable, report that explicitly. Check executable bits and update
[Included scenarios](#included-scenarios).
