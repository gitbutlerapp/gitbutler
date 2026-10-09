# Worktree benchmark

Build with `cargo bench -p but-workspace --features worktree-cow --bench worktrees --no-run`
for the workspace's gix feature set. Add `gix/max-performance` to the feature list
to match the gix CLI/desktop performance bundle. Record which variant is measured;
parallelism alone does not enable default pack caching.
Run the resulting executable directly; Cargo's benchmark argument is not needed.

Set `WORKTREE_BENCH_TMPDIR` to a dedicated case-sensitive APFS image mount.
`WORKTREE_BENCH_REPO` defaults to Byron's local Linux repository;
`WORKTREE_BENCH_RUNS=10`, `WORKTREE_BENCH_WARMUPS=2`, and
`WORKTREE_BENCH_LABEL=production` control repetitions and output labels.
The source repository is read only. A clean seed checkout borrows its object database.

Create a scratch image on macOS, outside measured operations:

```sh
hdiutil create -size 16g -type SPARSE -fs 'Case-sensitive APFS' \
  -volname WorktreeBenchmark -nospotlight /private/tmp/worktrees.sparseimage
mkdir -p /private/tmp/worktrees-mount
hdiutil attach /private/tmp/worktrees.sparseimage \
  -mountpoint /private/tmp/worktrees-mount -nobrowse
WORKTREE_BENCH_TMPDIR=/private/tmp/worktrees-mount target/release/deps/worktrees-<hash> \
  > /private/tmp/worktree-samples.jsonl
hdiutil detach /private/tmp/worktrees-mount
```

Save the pre-migration executable to compare both implementations on the same
seed and image. Alternate before/after invocations with one measured round per
invocation and collect JSONL samples outside the checkout. Record the results as
Markdown in the commit message, then discard temporary samples and logs.
Preparation, synchronization, disk accounting, validation and forced-removal
dirtiness are outside the timers.
Creation includes registration, branch creation, files and index; removal includes
safety checks and recursive deletion. Branch cleanup is untimed. Registration-only
worktrees need forced removal even without injected dirtiness.

Effective disk usage is the isolated volume's change in free blocks after volume
synchronization. It includes checkout/private administration and incremental shared
Git metadata. Unlike `du`, it accounts for partial sharing and filesystem metadata.
An allocation calibration checks a full write, APFS clone, and partial overwrite.
Logical bytes are reported separately. Keep other writers off the image.

Measurements describe warm-cache macOS APFS-image performance, not durable-write
latency or native-volume/cold-cache performance. COW on Linux is a full-copy test
mock and cannot produce valid APFS comparison numbers.

The explicitly invoked concurrency regression is:

```sh
cargo test -p but-workspace remove_finishes_when_a_bounded_writer \
  -- --ignored --nocapture
```

Before migration, 32 writes arriving after deletion of the marker reproduced
`Directory not empty`: checkout remained, while administration was already gone.
This stress is scheduling-sensitive; upstream gix also has deterministic coverage
injecting one late write after scanning each root.
