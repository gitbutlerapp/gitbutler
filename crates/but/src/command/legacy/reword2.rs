use bstr::{BStr, BString};
use but_api::{
    WorkspaceState,
    diff::ComputeLineStats,
    json::{ChangeIdString, HexHash},
};
use but_core::{DryRun, RefMetadata, diff::CommitDetails, sync::RepoExclusive};
use but_ctx::Context;
use but_error::Code;
use but_transaction::Transaction;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gix::{prelude::ObjectIdExt as _, refs::FullName};
use serde::Serialize;

use crate::{
    CliError, CliResult, IdMap,
    args::{
        atoms::{BranchOrCommit, Purpose},
        reword2::Platform,
    },
    bad_input,
    command::legacy::{
        ShowDiffInEditor,
        commit_message_prep::{normalize_commit_message, should_update_commit_message},
        reword::get_commit_message_from_editor as get_commit_message_from_editor_legacy,
    },
    id::CommitId,
    theme::{self, Theme},
    utils::{
        CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils,
        merged_upstream::MergedUpstream,
    },
};

#[must_use]
pub enum RewordOutcome {
    CommitUpdated {
        target: CommitId,
        new_commit: CommitId,
    },
    CommitUnchanged {
        target: CommitId,
    },
    BranchRenamed {
        old_name: FullName,
        new_name: String,
    },
    BranchUnchanged {
        name: FullName,
    },
}

impl CliOutputHuman for RewordOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &Theme,
    ) -> anyhow::Result<()> {
        match self {
            RewordOutcome::CommitUpdated {
                target,
                new_commit: _,
            } => writeln!(out, "Updated commit message for {}", theme::Commit(target))?,
            RewordOutcome::CommitUnchanged { target: _ } => {
                writeln!(out, "No changes to commit message")?
            }
            RewordOutcome::BranchRenamed { old_name, new_name } => writeln!(
                out,
                "Renamed {} to {}",
                theme::Branch(old_name),
                theme::Branch(&*new_name)
            )?,
            RewordOutcome::BranchUnchanged { name } => {
                writeln!(out, "Branch already named {}", theme::Branch(name))?
            }
        }
        Ok(())
    }
}

impl CliOutput for RewordOutcome {
    fn on_json(self) -> impl Serialize {
        #[derive(Serialize)]
        #[serde(
            tag = "type",
            rename_all = "camelCase",
            rename_all_fields = "camelCase"
        )]
        enum Output {
            CommitUpdated {
                changed: bool,
                source_commit_id: HexHash,
                #[serde(skip_serializing_if = "Option::is_none")]
                source_change_id: Option<ChangeIdString>,
                new_commit_id: HexHash,
                #[serde(skip_serializing_if = "Option::is_none")]
                new_change_id: Option<ChangeIdString>,
            },
            CommitUnchanged {
                changed: bool,
                commit_id: HexHash,
                #[serde(skip_serializing_if = "Option::is_none")]
                change_id: Option<ChangeIdString>,
            },
            BranchRenamed {
                changed: bool,
                old_branch_name: String,
                new_branch_name: String,
            },
            BranchUnchanged {
                changed: bool,
                branch_name: String,
            },
        }

        match self {
            RewordOutcome::CommitUpdated { target, new_commit } => Output::CommitUpdated {
                changed: true,
                source_commit_id: target.commit_id.into(),
                source_change_id: target.change_id.map(Into::into),
                new_commit_id: new_commit.commit_id.into(),
                new_change_id: new_commit.change_id.map(Into::into),
            },
            RewordOutcome::CommitUnchanged { target } => Output::CommitUnchanged {
                changed: false,
                commit_id: target.commit_id.into(),
                change_id: target.change_id.map(Into::into),
            },
            RewordOutcome::BranchRenamed { old_name, new_name } => Output::BranchRenamed {
                changed: true,
                old_branch_name: old_name.shorten().to_string(),
                new_branch_name: new_name,
            },
            RewordOutcome::BranchUnchanged { name } => Output::BranchUnchanged {
                changed: false,
                branch_name: name.shorten().to_string(),
            },
        }
    }
}

pub fn reword(
    ctx: &mut Context,
    mut out: IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<(RewordOutcome, Option<WorkspaceState>)> {
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let id_map = IdMap::new_from_context(ctx, guard.read_permission())?;
    let merged = MergedUpstream::from_ctx(ctx, args.allow_merged)?;
    let operation = resolve(ctx, &mut out, &id_map, args, &merged)?;

    run(ctx, &mut meta, guard.write_permission(), operation)
}

fn resolve(
    ctx: &mut Context,
    _out: &mut IntermediateChannel<'_>,
    id_map: &IdMap,
    args: Platform,
    merged: &MergedUpstream,
) -> CliResult<RewordOperation> {
    let Platform {
        target,
        message,
        format,
        allow_merged: _,
    } = args;

    let target = {
        let repo = ctx.repo.get()?;
        target.resolve_in_workspace(&repo, id_map, Purpose::Target, None)?
    };

    let operation = match target.into_branch_or_commit()? {
        BranchOrCommit::Commit(target) => {
            merged.ensure_commit_not_merged(target.commit_id)?;
            match (format, message) {
                (true, None) => RewordOperation::FormatCommit { target },
                (false, Some(message)) => RewordOperation::Commit {
                    target,
                    new_message: CommitMessageSource::Provided(message),
                },
                (false, None) => RewordOperation::Commit {
                    target,
                    new_message: CommitMessageSource::Editor { initial: None },
                },
                (true, Some(_)) => {
                    return Err(bad_input(
                        "--fix-formatting and --message cannot be used at the same time",
                    )
                    .into());
                }
            }
        }
        BranchOrCommit::Branch(branch) => {
            if format {
                return Err(bad_input(
                    "--fix-formatting flag can only be used with commits, not branches",
                )
                .into());
            }
            let ref_name = branch.resolve_local_branch_name()?;
            merged.ensure_branch_not_merged(ref_name.as_ref())?;
            let new_name = match message {
                Some(message) => BranchNameSource::Provided(message),
                None => BranchNameSource::Editor { initial: None },
            };
            RewordOperation::Branch {
                target: ref_name,
                new_name,
            }
        }
    };

    Ok(operation)
}

pub(crate) fn get_branch_name_from_editor(current_name: &str) -> anyhow::Result<String> {
    let mut template = String::new();
    template.push_str(current_name);
    if !current_name.is_empty() && !current_name.ends_with('\n') {
        template.push('\n');
    }
    template.push_str("\n# Please enter the new branch name. Lines starting\n");
    template.push_str("# with '#' will be ignored, and an empty name aborts the operation.\n");
    template.push_str("#\n");

    let branch_name =
        crate::tui::get_text::from_editor_no_comments("branch_name", &template)?.to_string();
    let branch_name = branch_name.trim();
    if branch_name.is_empty() {
        anyhow::bail!("Aborting due to empty branch name");
    }
    Ok(branch_name.to_owned())
}

pub fn run(
    ctx: &mut Context,
    meta: &mut impl RefMetadata,
    perm: &mut RepoExclusive,
    operation: RewordOperation,
) -> CliResult<(RewordOutcome, Option<WorkspaceState>)> {
    match operation {
        RewordOperation::Commit {
            target,
            new_message: message,
        } => {
            let commit_details = {
                let repo = ctx.repo.get()?;
                CommitDetails::from_commit_id(
                    target.commit_id.attach(&repo),
                    ComputeLineStats::No.into(),
                )?
            };
            let current_message = commit_details.commit.inner.message.to_string();
            let new_message = match message {
                CommitMessageSource::Empty => Some(String::new()),
                CommitMessageSource::Provided(message) => {
                    Some(normalize_commit_message(&message).to_owned())
                }
                CommitMessageSource::Editor { initial: current } => {
                    let repo = ctx.repo.get()?;
                    edit_commit_message(
                        &repo,
                        ctx.settings.context_lines,
                        commit_details,
                        current.as_deref().unwrap_or(&current_message),
                        &current_message,
                    )?
                }
            };
            let (outcome, ws) =
                reword_commit(ctx, meta, perm, target, &current_message, new_message)?;
            Ok((outcome, ws))
        }
        RewordOperation::FormatCommit { target } => {
            let current_message = {
                let repo = ctx.repo.get()?;
                let commit_details = CommitDetails::from_commit_id(
                    target.commit_id.attach(&repo),
                    ComputeLineStats::No.into(),
                )?;
                commit_details.commit.inner.message.to_string()
            };
            let new_message = Some(but_action::commit_format::format_commit_message(
                &current_message,
            ));
            let (outcome, ws) =
                reword_commit(ctx, meta, perm, target, &current_message, new_message)?;
            Ok((outcome, ws))
        }
        RewordOperation::Branch {
            target: old_name,
            new_name,
        } => Ok((
            match reword_branch(ctx, old_name.clone(), new_name, perm)? {
                BranchRename::Unchanged => RewordOutcome::BranchUnchanged { name: old_name },
                BranchRename::Renamed(new_name) => {
                    RewordOutcome::BranchRenamed { old_name, new_name }
                }
            },
            None,
        )),
    }
}

fn edit_commit_message(
    repo: &gix::Repository,
    context_lines: u32,
    commit_details: CommitDetails,
    editor_initial_message: &str,
    current_message_for_comparison: &str,
) -> anyhow::Result<Option<String>> {
    get_commit_message_from_editor_legacy(
        repo,
        context_lines,
        commit_details,
        editor_initial_message.to_owned(),
        current_message_for_comparison,
        ShowDiffInEditor::Unspecified,
    )
    .map_err(|err| {
        if let Some(Code::EditorExitedWithNonZeroStatus) = err.downcast_ref::<but_error::Code>() {
            anyhow::anyhow!("Editor exited with non-zero status")
        } else {
            err
        }
    })
}

pub enum RewordOperation {
    Commit {
        target: CommitId,
        new_message: CommitMessageSource,
    },
    FormatCommit {
        target: CommitId,
    },
    Branch {
        target: FullName,
        new_name: BranchNameSource,
    },
}

#[derive(Debug, Clone)]
pub enum CommitMessageSource {
    Empty,
    Provided(String),
    Editor {
        /// Override the initial text shown in the editor.
        ///
        /// If `None` the target's current message will be shown.
        initial: Option<String>,
    },
}

pub enum BranchNameSource {
    Provided(String),
    Editor {
        /// Override the initial text shown in the editor.
        ///
        /// If `None` the target's current name will be shown.
        initial: Option<String>,
    },
}

impl RewordOperation {
    #[expect(dead_code)]
    fn will_open_editor(&self) -> bool {
        match self {
            RewordOperation::Commit { new_message, .. } => new_message.will_open_editor(),
            RewordOperation::FormatCommit { .. } => false,
            RewordOperation::Branch { new_name, .. } => new_name.will_open_editor(),
        }
    }
}

impl BranchNameSource {
    fn will_open_editor(&self) -> bool {
        match self {
            BranchNameSource::Provided(_) => false,
            BranchNameSource::Editor { .. } => true,
        }
    }
}

impl CommitMessageSource {
    /// Check if this message source will open an editor.
    ///
    /// Used by the TUI to suspend itself.
    pub fn will_open_editor(&self) -> bool {
        match self {
            CommitMessageSource::Editor { .. } => true,
            CommitMessageSource::Empty | CommitMessageSource::Provided(_) => false,
        }
    }

    /// Resolve mutually exclusive commit-message arguments into a message source.
    pub fn from_args(no_message: bool, message: Option<Vec<String>>) -> CliResult<Self> {
        match (no_message, message) {
            (true, None) => Ok(Self::Empty),
            (false, None) => Ok(Self::Editor { initial: None }),
            (false, Some(message)) => Ok(Self::Provided(message.join("\n\n"))),
            (true, Some(_)) => {
                Err(bad_input("--no-message and --message cannot be used at the same time").into())
            }
        }
    }

    pub fn execute(
        self,
        new_commit: CommitId,
        tx: &mut Transaction<'_, '_, impl RefMetadata>,
    ) -> anyhow::Result<CommitId> {
        let message = match self {
            CommitMessageSource::Empty => Some(String::new()),
            CommitMessageSource::Provided(message) => Some(message),
            CommitMessageSource::Editor { initial: current } => {
                let repo = tx.repo();
                let commit_details = CommitDetails::from_commit_id(
                    new_commit.commit_id.attach(repo),
                    ComputeLineStats::No.into(),
                )?;

                let current_message = commit_details.commit.inner.message.to_string();

                edit_commit_message(
                    tx.repo(),
                    tx.context_lines(),
                    commit_details,
                    current.as_deref().unwrap_or(&current_message),
                    &current_message,
                )?
            }
        };

        let Some(message) = message else {
            return Ok(new_commit);
        };

        let reworded_commit =
            tx.reword_commit(new_commit.commit_id, BString::from(message).as_ref())?;

        Ok(reworded_commit.into())
    }
}

fn reword_commit(
    ctx: &mut Context,
    meta: &mut impl RefMetadata,
    perm: &mut RepoExclusive,
    target: CommitId,
    current_message: &str,
    new_message: Option<String>,
) -> anyhow::Result<(RewordOutcome, Option<WorkspaceState>)> {
    let Some(new_message) =
        new_message.filter(|message| should_update_commit_message(current_message, message))
    else {
        return Ok((RewordOutcome::CommitUnchanged { target }, None));
    };

    let snapshot_details = SnapshotDetails::new(OperationKind::UpdateCommitMessage);
    let (new_commit, ws) = but_transaction::with_transaction_with_perm(
        ctx,
        meta,
        perm,
        snapshot_details,
        DryRun::No,
        |mut tx| {
            let new_commit =
                tx.reword_commit(target.commit_id, BString::from(new_message).as_ref())?;
            Ok(but_transaction::Commit(new_commit))
        },
    )?;

    Ok((
        RewordOutcome::CommitUpdated {
            target,
            new_commit: new_commit.into(),
        },
        Some(ws),
    ))
}

fn reword_branch(
    ctx: &mut Context,
    ref_name: FullName,
    new_name: BranchNameSource,
    perm: &mut RepoExclusive,
) -> CliResult<BranchRename> {
    let current_name = ref_name.shorten().to_string();
    let new_name = match new_name {
        BranchNameSource::Provided(name) => {
            let name = name.trim();
            if name.is_empty() {
                return Err(anyhow::anyhow!("Aborting due to empty branch name").into());
            }
            name.to_owned()
        }
        BranchNameSource::Editor { initial: current } => {
            get_branch_name_from_editor(current.as_deref().unwrap_or(&current_name))?
        }
    };
    let new_name = validate_branch_name(new_name)?;

    let result = but_api::branch::branch_rename_with_perm(ctx, ref_name.clone(), new_name, perm)?;
    if result.new_ref.as_ref() == ref_name.as_ref() {
        Ok(BranchRename::Unchanged)
    } else {
        Ok(BranchRename::Renamed(result.new_ref.shorten().to_string()))
    }
}

fn validate_branch_name(name: String) -> CliResult<String> {
    let normalized = but_core::branch::normalize_short_name(name.as_str()).map_err(|err| {
        CliError::from(bad_input(format!("Invalid branch name: {err}")).arg_value(&name))
    })?;
    if normalized != <&BStr>::from(name.as_str()) {
        return Err(bad_input("Invalid branch name")
            .arg_value(&name)
            .hint(format!("Try '{normalized}' instead"))
            .into());
    }
    Ok(name)
}

#[derive(Debug)]
enum BranchRename {
    Unchanged,
    Renamed(String),
}
