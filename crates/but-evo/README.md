# But-evo

This file serves as an initial spec and what needs to be a stable definition. If
you are an agent, please be very very cautious when making changes. Make sure
your human understands the consiquences.

## Plain structure:

Obsolescence markers use the following data structure:

```rs
/// An obsolescence marker. hg also has a "flags" field, which we leave out.
/// `usingsha256` can be inferred from the width of the object IDs. `bumpedfix`
/// only matters for phase divergence, which we're not interested in.
struct Marker {
    prec: gix::ObjectId,
    succs: Vec<gix::ObjectId>,
    meta: BTreeMap<BString, BString>,
    date: gix::date::Time,
    // HG only writes parents for a prune, but I don't see why we shouldn't
    // always write them. Their main job is anchoring a prune, which has no
    // successor, to a commit that exchange can reach. Writing them on every
    // marker also keeps a record of where `prec` sat after `git gc` removes it.
    //
    // `None` means not recorded, `Some(vec![])` means `prec` is a root commit.
    parents: Option<Vec<gix::ObjectId>>,
}
```

Typical metadata:

- user - always written
- operation - what operation caused the rewrite
- ef1 - flags to indicate what changed, as decimal in bstring. The flags are ORed
  across all `succs`, and a prune is always `0`.
    - 1, description: the conflict markers GitButler adds to a conflicted commit's
      message don't count as a change
    - 2, extra_metadata: extra headers, ignoring `gpgsig`, `gpgsig-sha256` and
      `gitbutler-headers-version`.
    - 4, parents
    - 8, diff: what each commit introduces over its first parent differs, compared
      with one line of context and no line numbers.
    - 16, user: the author's name or email
    - 32, date: the author date changed.
    - 64, branch: unused, only relevant in hg with their branch coloring.
- note - free text
- If it's part of a fold:
    - fold-id: 8 lower hex chars in a bstring. This is the first 8 hex chars of
      the SHA-1 of `user`, followed by each `prec` and then the `succ`. hg also
      hashes local revision numbers, which Git doesn't have, so it's opaque and
      must never be recomputed for comparison.
    - fold-idx: as decimal in bstring, starting at 1
    - fold-size: as decimal in bstring

A fold only ever has one successor.

### Examples:

#### Fold aa11 and bb22 into cc33:

```
Marker {
    prec: aa11,
    succs: [cc33],
    meta: [
        (b"ef1", ...),
        (b"fold-id", b"abababab"),
        (b"fold-idx", b"1"),
        (b"fold-size", b"2"),
        (b"operation", b"editor op"),
        (b"user", b"Caleb Owens <caleb@gitbutler.com>"),
    ],
    date: some time...,
    parents: parents of aa11
}

Marker {
    prec: bb22,
    succs: [cc33],
    meta: [
        (b"ef1", ...),
        (b"fold-id", b"abababab"),
        (b"fold-idx", b"2"),
        (b"fold-size", b"2"),
        (b"operation", b"editor op"),
        (b"user", b"Caleb Owens <caleb@gitbutler.com>"),
    ],
    date: some time...,
    parents: parents of bb22
}
```

#### Split cc33 into aa11 and bb22:

```
Marker {
    prec: cc33,
    succs: [aa11, bb22],
    meta: [
        (b"ef1", ...),
        (b"operation", b"editor op"),
        (b"user", b"Caleb Owens <caleb@gitbutler.com>"),
    ],
    date: some time...,
    parents: parents of cc33
}
```

#### Drop/Prune aa11:

```
Marker {
    prec: aa11,
    succs: [],
    meta: [
        (b"ef1", b"0"),
        (b"operation", b"editor op"),
        (b"user", b"Caleb Owens <caleb@gitbutler.com>"),
    ],
    date: some time...,
    parents: parents of aa11
}
```

#### Amend aa11:

Changing only the content of the commit:

```
Marker {
    prec: aa11,
    succs: [aa11'],
    meta: [
        (b"ef1", b"8"),
        (b"operation", b"editor op"),
        (b"user", b"Caleb Owens <caleb@gitbutler.com>"),
    ],
    date: some time...,
    parents: parents of aa11
}
```

#### Diverge aa11:

No single operation writes this. It's what two independent rewrites of aa11 look
like, for example on two machines, once their markers have been merged.

```
Marker {
    prec: aa11,
    succs: [bb22],
    meta: [
        (b"ef1", ...),
        (b"operation", b"editor op"),
        (b"user", b"Caleb Owens <caleb@gitbutler.com>"),
    ],
    date: some time...,
    parents: parents of aa11
}

Marker {
    prec: aa11,
    succs: [cc33],
    meta: [
        (b"ef1", ...),
        (b"operation", b"editor op"),
        (b"user", b"Caleb Owens <caleb@gitbutler.com>"),
    ],
    date: some time...,
    parents: parents of aa11
}
```

## Storage in GitButler

We want to have obs logs to be stored in GitButler's sqlite database, as well as
other implementations on the server side.

We will want to have some trait that has the following APIs:

- find_by_prec :: gix::ObjectId -> Result<Vec<Marker>>
- find_by_succ :: gix::ObjectId -> Result<Vec<Marker>>
- write :: [Marker] -> Result<()>

`write` must be atomic, and must ignore markers that are already stored.

Storage implementations will be expected to find matching markers in at most
`O(log n + k)` time, where `k` is the number of matches.

### SQLITE

For the primary client, we will store this in our sqlite database with the
following format:

```sql
CREATE TABLE `obs_markers`(
    `id` BLOB NOT NULL PRIMARY KEY,
    `prec` BLOB NOT NULL,
    `date_seconds` INTEGER NOT NULL,
    `date_offset` INTEGER NOT NULL,
    `parents_recorded` INTEGER NOT NULL
);
CREATE INDEX `idx_obs_markers_prec` ON `obs_markers`(`prec`);

CREATE TABLE `obs_marker_succs`(
    `marker_id` BLOB NOT NULL,
    `position` INTEGER NOT NULL,
    `succ` BLOB NOT NULL,
    PRIMARY KEY (`marker_id`, `position`),
    FOREIGN KEY (`marker_id`) REFERENCES `obs_markers`(`id`)
);
CREATE INDEX `idx_obs_marker_succs_succ` ON `obs_marker_succs`(`succ`);

CREATE TABLE `obs_marker_parents`(
    `marker_id` BLOB NOT NULL,
    `position` INTEGER NOT NULL,
    `parent` BLOB NOT NULL,
    PRIMARY KEY (`marker_id`, `position`),
    FOREIGN KEY (`marker_id`) REFERENCES `obs_markers`(`id`)
);

CREATE TABLE `obs_marker_meta`(
    `marker_id` BLOB NOT NULL,
    `key` BLOB NOT NULL,
    `value` BLOB NOT NULL,
    PRIMARY KEY (`marker_id`, `key`),
    FOREIGN KEY (`marker_id`) REFERENCES `obs_markers`(`id`)
);
```

- `id` exists only for storage. It's the SHA-1 of a length-prefixed encoding of
  every marker field, which is what lets a marker with variable-length `succs`,
  `parents` and `meta` have a primary key. The encoding must distinguish `None`
  from empty `parents`, and can never change, or the same marker would stop
  deduplicating against rows that are already stored. The encoding is, in order:
    - `prec` as bytes
    - `succs` as a list
    - the number of `meta` entries as a length, then each key and value as bytes,
      in key order
    - `date` as a big-endian `i64` of seconds and a big-endian `i32` offset
    - `parents`, as the byte `0` for `None`, or the byte `1` and then a list

  A length is a big-endian `u64`, bytes are a length and then the raw bytes, and a
  list is a length and then each ID as bytes.
- `write` uses `INSERT OR IGNORE` into `obs_markers` inside one immediate
  transaction, and skips the side tables for a marker that was already present.
- `position` preserves the order of `succs` and `parents`, which is part of a
  marker's value. Parent order also distinguishes `p1` from `p2`.
- Each find loads the matching ids, then the side table rows for each id through
  their primary keys.
- The tables live in `but.sqlite` through `DbHandle`, not in the project cache,
  because markers can't be regenerated. They follow the `crates/but-db/src/table`
  pattern, with an `M::up` migration using `SchemaVersion::Zero`, as new tables
  don't affect older binaries.

`but-db` keeps its own row type like its other tables. `but-evo` depends on
`but-db`, and implements the trait for `DbHandle`, as `write` starts the immediate
transaction, which a table handle can't do. `but-rebase` depends on `but-evo`.

## Primary interface

### Describing rewrites

An operation describes what it did as `Rewrite`s, which `but-evo` turns into
markers. This makes an invalid shape, like a fold with two successors,
unrepresentable.

```rs
enum Rewrite {
    Replace { prec: gix::ObjectId, succ: gix::ObjectId },
    Split { prec: gix::ObjectId, succs: Vec<gix::ObjectId> },
    Fold { precs: Vec<gix::ObjectId>, succ: gix::ObjectId },
    Prune { prec: gix::ObjectId },
}

struct Provenance {
    user: BString,
    date: gix::date::Time,
    operation: Option<BString>,
}

/// Writes one marker per `Replace`, `Split` and `Prune`, and one per `prec` of a
/// `Fold`. `repo` provides the parents and the commits to compute `ef1` from.
fn record(
    store: &mut impl MarkerStore,
    repo: &gix::Repository,
    rewrites: &[Rewrite],
    provenance: &Provenance,
) -> Result<()>;
```

A `prec` that is also a successor survived the rewrite, as with a squash that left
its target identical, so it gets no marker. A `Fold` left with one `prec` that way
has no `fold-*` metadata.

