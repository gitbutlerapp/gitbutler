use std::fmt::Display;

use anyhow::Context;
use bstr::{BStr, BString, ByteSlice};
use but_api::{diff::ComputeLineStats, json::HexHash};
use but_core::{DiffSpec, DryRun, RefMetadata, diff::CommitDetails, ref_metadata::StackId};
use but_error::Code;
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use but_transaction::{DynamicOutcome, IntermediateCommitCreateResult, Transaction};
use but_workspace::RefInfo;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gix::{prelude::ObjectIdExt as _, refs::FullName};
use serde::Serialize;

use crate::{
    CliResult, CliResultExt,
    args::{atoms::BranchArg, commit2::Platform},
    bad_input,
    command::legacy::{ShowDiffInEditor, reword::get_commit_message_from_editor},
    id::UNASSIGNED,
    theme::{Paint, Theme},
    utils::{
        CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils, diff_specs::DiffSpecBuilder,
    },
};

#[must_use]
pub struct CommitOutcome {
    new_commit: gix::ObjectId,
    ref_name: gix::refs::FullName,
}

impl CliOutputHuman for CommitOutcome {
    fn on_human(self, out: &mut dyn WriteWithUtils, _theme: &Theme) -> anyhow::Result<()> {
        let Self {
            new_commit,
            ref_name,
        } = self;
        writeln!(
            out,
            "Created commit {} on {}",
            Commit(new_commit),
            Branch(ref_name.shorten()),
        )?;
        Ok(())
    }
}

impl CliOutput for CommitOutcome {
    fn on_shell(self, out: &mut dyn WriteWithUtils) -> anyhow::Result<()> {
        let Self {
            new_commit,
            ref_name: _,
        } = self;
        writeln!(out, "{}", new_commit.to_hex_with_len(7))?;
        Ok(())
    }

    fn on_json(self) -> impl serde::Serialize {
        #[derive(Serialize)]
        struct Output {
            commit: HexHash,
        }

        Output {
            commit: self.new_commit.into(),
        }
    }
}

pub fn commit(
    ctx: &mut but_ctx::Context,
    _out: IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<CommitOutcome> {
    let Platform {
        no_message,
        message,
        branch,
    } = args;

    let mut guard = ctx.exclusive_worktree_access();
    let perm = guard.write_permission();

    let mut meta = ctx.meta()?;
    let (changes, operation) = {
        let context_lines = ctx.settings.context_lines;
        let (repo, ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
        let head_info = but_workspace::head_info(&repo, &meta, Default::default())?;

        let operation = route_operation(&repo, &head_info, branch)?;

        let mut builder = DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);
        builder.push_changes_from_unassigned(&UNASSIGNED.to_string())?;
        let changes = builder.into_diff_specs();

        (changes, operation)
    };

    let snapshot_details = SnapshotDetails::new(OperationKind::CreateCommit);
    let result = but_transaction::with_transaction_with_perm(
        ctx,
        &mut meta,
        perm,
        snapshot_details,
        DryRun::No,
        |mut tx| {
            let (
                IntermediateCommitCreateResult {
                    new_commit,
                    rejected_specs,
                },
                ref_name,
            ) = operation.execute(&mut tx, changes)?;

            anyhow::ensure!(rejected_specs.is_empty(), "Couldn't commit all changes");

            let new_commit =
                new_commit.context("BUG: rejected_specs is empty yet nothing was committed")?;

            let message = match (no_message, message) {
                (true, None) => String::new(),
                (false, None) => {
                    let repo = tx.repo();
                    let commit_details = CommitDetails::from_commit_id(
                        new_commit.attach(repo),
                        ComputeLineStats::No.into(),
                    )?;

                    get_commit_message_from_editor(
                        tx.repo(),
                        tx.context_lines(),
                        commit_details,
                        String::new(),
                        "",
                        ShowDiffInEditor::Unspecified,
                    )?
                    .unwrap_or_default()
                }
                (false, Some(message)) => message,
                (true, Some(_)) => {
                    unreachable!("--no-message and --message are mutually exclusive")
                }
            };

            let reworded_commit = tx.reword_commit(new_commit, BString::from(message).as_ref())?;

            Ok(DynamicOutcome::<_, std::convert::Infallible>::Commit((
                reworded_commit,
                ref_name,
            )))
        },
    );

    let DynamicOutcome::Commit(((new_commit, ref_name), _ws)) = match result {
        Ok(outcome) => outcome,
        Err(err) => {
            return Err(
                if let Some(Code::EditorExitedWithNonZeroStatus) =
                    err.downcast_ref::<but_error::Code>()
                {
                    bad_input("Editor exited with non-zero status").into()
                } else {
                    err.into()
                },
            );
        }
    };

    Ok(CommitOutcome {
        new_commit,
        ref_name,
    })
}

fn route_operation(
    repo: &gix::Repository,
    head_info: &RefInfo,
    branch: Option<BranchArg>,
) -> CliResult<CommitOperation> {
    if let Some(branch) = branch {
        if let Some(segment) = branch.try_resolve_segment(head_info)? {
            let ref_info = segment.ref_info.with_context(|| {
                format!("BUG: Segment resolved from branch name {branch} has no ref info")
            })?;

            return Ok(CommitOperation::CommitToExistingBranch(
                CommitToExistingBranchOperation {
                    branch_name: ref_info.ref_name,
                },
            ));
        } else {
            let branch_name = BranchArg(branch.resolve_for_creation(repo, head_info).hint(
                format!("Run `but apply {branch}` to apply the branch first"),
            )?)
            .resolve_local_branch_name()?;
            return Ok(CommitOperation::CommitToNewBranch(
                CommitToNewBranchOperation {
                    branch_name: Some(branch_name),
                },
            ));
        }
    }

    let mut stacks = head_info.stacks.iter();
    if let Some(stack) = stacks.next() {
        if stacks.next().is_some() {
            return Err(anyhow::anyhow!("Found more than one stack, badness!").into());
        }

        let ref_info = stack
            .segments
            .first()
            .and_then(|segment| segment.ref_info.as_ref())
            .context("Head stack as no ref")?;

        return Ok(CommitOperation::CommitToExistingBranch(
            CommitToExistingBranchOperation {
                branch_name: ref_info.ref_name.clone(),
            },
        ));
    }

    Ok(CommitOperation::CommitToNewBranch(
        CommitToNewBranchOperation { branch_name: None },
    ))
}

enum CommitOperation {
    CommitToExistingBranch(CommitToExistingBranchOperation),
    CommitToNewBranch(CommitToNewBranchOperation),
}

impl CommitOperation {
    fn execute(
        self,
        tx: &mut Transaction<'_, '_, impl RefMetadata>,
        changes: Vec<DiffSpec>,
    ) -> anyhow::Result<(IntermediateCommitCreateResult, FullName)> {
        match self {
            CommitOperation::CommitToExistingBranch(op) => op.execute(tx, changes),
            CommitOperation::CommitToNewBranch(op) => op.execute(tx, changes),
        }
    }
}

struct CommitToExistingBranchOperation {
    branch_name: gix::refs::FullName,
}

impl CommitToExistingBranchOperation {
    fn execute(
        self,
        tx: &mut Transaction<'_, '_, impl RefMetadata>,
        changes: Vec<DiffSpec>,
    ) -> anyhow::Result<(IntermediateCommitCreateResult, FullName)> {
        let Self {
            branch_name: ref_name,
        } = self;

        let commit_create_result = tx.create_commit(
            RelativeTo::Reference(ref_name.clone()),
            InsertSide::Below,
            changes,
            String::new(),
        )?;

        Ok((commit_create_result, ref_name))
    }
}

struct CommitToNewBranchOperation {
    branch_name: Option<FullName>,
}

impl CommitToNewBranchOperation {
    fn execute(
        self,
        tx: &mut Transaction<'_, '_, impl RefMetadata>,
        changes: Vec<DiffSpec>,
    ) -> anyhow::Result<(IntermediateCommitCreateResult, FullName)> {
        let Self { branch_name } = self;

        let branch_name = if let Some(branch_name) = branch_name {
            branch_name
        } else {
            but_core::branch::unique_canned_refname(tx.repo())?
        };

        tx.create_reference(branch_name.as_ref(), None, |_| StackId::generate(), None)?;

        let commit_create_result = tx.create_commit(
            RelativeTo::Reference(branch_name.clone()),
            InsertSide::Below,
            changes,
            String::new(),
        )?;

        Ok((commit_create_result, branch_name))
    }
}

struct Commit(gix::ObjectId);

impl Display for Commit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let t = crate::theme::get();
        write!(
            f,
            "{}",
            t.commit_id.paint(self.0.to_hex_with_len(7).to_string())
        )
    }
}

struct Branch<'a>(&'a BStr);

impl Display for Branch<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let t = crate::theme::get();
        write!(f, "'{}'", t.local_branch.paint(self.0.to_str_lossy()))
    }
}
