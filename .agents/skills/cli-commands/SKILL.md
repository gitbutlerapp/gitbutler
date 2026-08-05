---
name: cli-commands
description: Use when adding or modifying CLI command (`but` commands) under `crates/but/src`.
---

Imagine we're implementing a new `commit3` command. The high-level structure
for that must be as follows.

## Arguments

Arguments live in `crates/but/src/args`. For our commit command that would be
`crates/but/src/args/commit3.rs`:

```rust
use crate::args::atoms::CliIdArg;

/// Create a commit.
///
/// More details about the command here...
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
#[deny(missing_docs)]
pub struct Platform {
    /// The message to use for the commit.
    #[clap(short, long, group = "commit_message")]
    pub message: Option<Vec<String>>,

    /// Place the commit on the branch `BRANCH`.
    #[clap(short, long, value_name = "BRANCH", group = "targeting")]
    pub branch: Option<Option<CliIdArg>>,

    /// One or more changes to commit.
    pub changes: Vec<CliIdArg>,
}
```

In `crates/but/src/args/mod.rs`:

```rust
#[cfg(feature = "legacy")]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
#[clap(hide = true, name = "_commit3")]
Commit3(commit3::Platform),
```

Things to note:

Arguments that refer to Git objects (such as commits, branches, files, hunks,
etc.) use some type from `crates/but/src/args/atoms/` and not `String` or another
loose type.

Use `CliIdArg` for arguments that reference existing Git objects such as
branches, commits, files, etc. This allows the user to use short IDs or fully
qualified names.

`String` should only be used for truly loose text input such as commit
messages.

`Platform` and all of its fields must have documentation. The `Subcommands`
variant intentionally has no doc comment because clap obtains the command
documentation from `Platform`.

Use `#[clap(group = "...")]` to create mutually exclusive groups of arguments.

Commands with tricky grammar can define a `pub(crate) const ERROR_EXAMPLES`
next to `Platform` and register it in `args::error_examples`; the block is
appended after clap parse errors. At most 4 lines, each a
`but <cmd> ...   # what it does` invocation that works as written (e.g.
include `-m` where omitting it would open an editor).

## Handling the command

Add a match arm to `crates/but/src/lib.rs` to handle the command:

```rust
match cmd {
    Subcommands::Commit3(commit_args) => {
        use crate::utils::IntermediateChannel;

        let status_after = args.status_after;
        let mut ctx = setup::init_ctx(
            &args,
            InitCtxOptions {
                background_sync: BackgroundSync::Enabled { silent: false },
                ..Default::default()
            },
            out,
        )?;
        out.begin_status_after(status_after);

        let outcome = command::legacy::commit3::commit(
            &mut ctx,
            IntermediateChannel::new(out),
            commit_args,
        )
        .emit_metrics(metrics_ctx)?;
        out.print_cli_output(outcome)?;

        run_status_after_if_requested(status_after, &mut ctx, out);

        Ok(())
    }

    // all the other commands...
}
```

Things to note:

Use `IntermediateChannel`. Do not pass `OutputChannel` to commands.

Use `OutputChannel::print_cli_output` to print the final output from the
command. This ensures we handle all supported formats. If only human format is
supported, use `OutputChannel::print_cli_output_human`.

## Implementing the command

In `crates/but/src/command/legacy/commit3.rs`

```
pub fn commit(
    ctx: &mut Context,
    out: IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<CommitOutcome> {
    // get whatever dependencies we need from `Context` such as
    // `RepoExclusiveGuard`, `IdMap`, `RefInfo`, etc.

    // resolve the arguments into a `CommitOperation`
    let commit_operation = resolve(ctx, args)?;

    // Run the operation
    let outcome = run(ctx, commit_operation)?;

    // Return the outcome which will be printed by the caller
    Ok(outcome)
}

fn resolve(ctx: &mut Context, args: Platform) -> CliResult<CommitOperation> {
    let Platform { message, branch, changes } = args;
    // ...
}

fn run(ctx: &mut Context, commit_op: CommitOperation) -> anyhow::Result<CommitOutcome> {
    match commit_op {
        // ...
    }
}

#[must_use]
struct CommitOutcome {
    new_commit: ObjectId,
}

impl CliOutputHuman for CommitOutcome {
    fn on_human(self, out: &mut dyn WriteWithUtils, _theme: &Theme) -> anyhow::Result<()> {
        let Self { new_commit } = self;

        writeln!(
            out,
            "Created commit {}",
            theme::Commit(new_commit, None),
        )?;

        Ok(())
    }
}

impl CliOutput for CommitOutcome {
    fn on_shell(self, out: &mut dyn WriteWithUtils) -> anyhow::Result<()> {
        let Self { new_commit } = self;

        writeln!(out, "{}", new_commit.to_hex_with_len(7))?;

        Ok(())
    }

    fn on_json(self) -> impl serde::Serialize {
        #[derive(Serialize)]
        struct Output {
            commit: HexHash,
        }

        let Self { new_commit } = self;

        Output { commit: new_commit.into() }
    }
}
```

Things to note:

Commands follow a `resolve` then `run` structure.

`run` doesn't print its final output. It returns something that implements
`CliOutput` / `CliOutputHuman` which the caller can then print.

`resolve` returns `CliResult` because it needs to reject bad user input.

`run` returns `anyhow::Result` because it can only hit internal errors. Bad
user input is handled by `resolve`.

`resolve` translates CLI arguments into domain targets and validates user
input. It may query repository state to disambiguate or reject input, but
should not retain derived data in the operation.

`run()` loads current repository state and computes results. Commit counts,
diffs, statistics, branch details, current tips, and workspace projections
generally belong in `run()`. State derivable from those identifiers and the
context belongs in `run()`.

The operation should not contain types from `crate::args::atoms`.

A non-CLI caller such as the TUI should be able to construct an operation from
domain identifiers without reproducing repository queries from `resolve()`. If
it must compute counts, diffs, or branch metadata to build the operation, those
fields probably belong in `run()`.

The operation does not contain diff specs (usually in the form
`Vec<DiffSpec>`). It should instead contain `CliId`s which `run` turns into
`Vec<DiffSpec>` using `DiffSpecBuilder`. This makes it easier for the status
TUI to call `run` directly.

User-input pickers and prompts are created via `InputOutputChannel`
accessed through `IntermediateChannel::prepare_for_terminal_input`.

The JSON output includes both commit IDs and change IDs.

Use `AllowMergedArg` and `MergedUpstream` to verify we don't mutate merged
commits and branches.

Printing of commits, branches, change IDs, etc. uses newtypes from `theme` such
as `theme::Branch` and `theme::Commit`. This ensures consistent coloring.

Bad user input errors use `bad_input(...)`, optionally with `.arg_name()`,
`.arg_value()`, and `.hint()`.

Avoid making the operation or outcome types implement `serde::Serialize`.
Define the specific types needed inside `fn on_json`. This makes it harder to
break compatibility by accident. Reusing `Serialize` from existing domain types
is fine.

## More examples

For examples of this structure in practice see

- `crates/but/src/command/legacy/commit.rs`
- `crates/but/src/command/legacy/move.rs`
- `crates/but/src/command/legacy/squash.rs`
- `crates/but/src/command/legacy/diff.rs`
