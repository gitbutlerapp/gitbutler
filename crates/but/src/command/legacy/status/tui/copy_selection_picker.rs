use std::{borrow::Cow, collections::BTreeSet};

use anyhow::Context as _;
use bstr::{BString, ByteSlice as _};
use but_core::{CommitOwned, TreeChange, diff::CommitDetails};
use but_ctx::Context;
use but_hunk_assignment::HunkAssignment;
use gix::{prelude::ObjectIdExt as _, refs::FullName};
use nonempty::NonEmpty;
use ratatui::style::Style;

use crate::{
    command::legacy::status::tui::{
        Col, FuzzyPicker, FuzzyPickerItem, Message, SearchableToken, ToastKind,
    },
    id::{ShortId, UncommittedHunkOrFile},
    theme::Theme,
};

pub(super) fn commit_picker(
    commit_id: gix::ObjectId,
    theme: &'static Theme,
) -> FuzzyPicker<CopySelectionItem> {
    picker(
        NonEmpty::from_slice(&[
            CopySelectionItem::CommitSha(commit_id),
            CopySelectionItem::ShortCommitSha(commit_id),
            CopySelectionItem::CommitMessageTitle(commit_id),
            CopySelectionItem::WholeCommitMessage(commit_id),
            CopySelectionItem::CommitAuthor(commit_id),
            CopySelectionItem::CommitDiff(commit_id),
        ])
        .unwrap(),
        theme,
    )
}

pub(super) fn branch_picker(
    branch: FullName,
    theme: &'static Theme,
) -> FuzzyPicker<CopySelectionItem> {
    picker(
        NonEmpty::from_slice(&[
            CopySelectionItem::BranchName(branch.clone()),
            CopySelectionItem::PullRequestUrl(branch.clone()),
            CopySelectionItem::BranchDiff(branch.clone()),
        ])
        .unwrap(),
        theme,
    )
}

pub(super) fn uncommitted_hunk_picker(
    hunk: UncommittedHunkOrFile,
    theme: &'static Theme,
) -> FuzzyPicker<CopySelectionItem> {
    let path = hunk.hunk_assignments.head.path.clone();
    picker(
        NonEmpty::from_slice(&[
            CopySelectionItem::ShortId(hunk.id.clone()),
            CopySelectionItem::HunkDiff(Box::new(hunk)),
            CopySelectionItem::FilePath(path),
        ])
        .unwrap(),
        theme,
    )
}

pub(super) fn committed_file_picker(
    path: BString,
    id: ShortId,
    theme: &'static Theme,
) -> FuzzyPicker<CopySelectionItem> {
    // TODO(david): also support copying the diff for the file
    picker(
        NonEmpty::from_slice(&[
            CopySelectionItem::ShortId(id),
            CopySelectionItem::FilePath(path.to_string()),
        ])
        .unwrap(),
        theme,
    )
}

fn picker(
    items: NonEmpty<CopySelectionItem>,
    theme: &'static Theme,
) -> FuzzyPicker<CopySelectionItem> {
    FuzzyPicker::new(items, theme, |item, ctx, messages| {
        arboard::Clipboard::new()
            .context("failed to initialize clipboard")?
            .set_text(item.what_to_copy(ctx)?)
            .context("failed to copy to system clipboard")?;

        messages.push(Message::ShowToast {
            kind: ToastKind::Info,
            text: format!("Copied {}", lowercase_first_letter(item.as_str())).into(),
        });

        Ok(())
    })
}

#[derive(Debug, Clone)]
pub(super) enum CopySelectionItem {
    // shared
    ShortId(ShortId),
    FilePath(String),

    // commits
    CommitSha(gix::ObjectId),
    ShortCommitSha(gix::ObjectId),
    CommitMessageTitle(gix::ObjectId),
    WholeCommitMessage(gix::ObjectId),
    CommitAuthor(gix::ObjectId),
    CommitDiff(gix::ObjectId),

    // branches
    BranchName(FullName),
    BranchDiff(FullName),
    PullRequestUrl(FullName),

    // uncommitted files/hunks
    HunkDiff(Box<UncommittedHunkOrFile>),
}

impl CopySelectionItem {
    fn as_str(&self) -> &'static str {
        match self {
            CopySelectionItem::CommitSha(_) => "Commit ID",
            CopySelectionItem::ShortCommitSha(_) => "Short commit ID",
            CopySelectionItem::CommitMessageTitle(_) => "Message title",
            CopySelectionItem::WholeCommitMessage(_) => "Whole message",
            CopySelectionItem::CommitAuthor(_) => "Author",
            CopySelectionItem::CommitDiff(_)
            | CopySelectionItem::BranchDiff(_)
            | CopySelectionItem::HunkDiff(_) => "Diff",
            CopySelectionItem::BranchName(_) => "Branch name",
            CopySelectionItem::PullRequestUrl(_) => "Pull Request URL",
            CopySelectionItem::ShortId(_) => "Short ID",
            CopySelectionItem::FilePath(_) => "File path",
        }
    }

    fn what_to_copy(&self, ctx: &Context) -> anyhow::Result<String> {
        let repo = ctx.repo.get()?;
        match self {
            CopySelectionItem::CommitSha(commit_id) => Ok(commit_id.to_string()),
            CopySelectionItem::ShortCommitSha(commit_id) => {
                Ok(commit_id.to_hex_with_len(7).to_string())
            }
            CopySelectionItem::CommitMessageTitle(commit_id) => {
                let commit = commit(&repo, *commit_id)?;
                let commit_message = commit.message.to_str_lossy();
                Ok(commit_message
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .to_string())
            }
            CopySelectionItem::WholeCommitMessage(commit_id) => {
                let commit = commit(&repo, *commit_id)?;
                let commit_message = commit.message.to_str_lossy();
                Ok(commit_message.to_string())
            }
            CopySelectionItem::CommitAuthor(commit_id) => {
                let commit = commit(&repo, *commit_id)?;
                let author = &commit.author;
                Ok(format!(
                    "{} <{}>",
                    author.name.to_str_lossy(),
                    author.email.to_str_lossy()
                ))
            }
            CopySelectionItem::CommitDiff(commit_id) => {
                let commit_details = commit_details(&repo, *commit_id)?;
                tree_changes_to_diff(
                    commit_details.diff_with_first_parent,
                    &repo,
                    ctx.settings.context_lines,
                )
            }
            CopySelectionItem::BranchName(branch_name) => {
                Ok(branch_name.shorten().to_str_lossy().to_string())
            }
            CopySelectionItem::BranchDiff(branch_name) => {
                let name = branch_name.shorten().to_str_lossy().to_string();
                let tree_changes = but_api::branch::branch_diff(ctx, name)?;
                tree_changes_to_diff(
                    tree_changes.changes.into_iter().map(Into::into).collect(),
                    &repo,
                    ctx.settings.context_lines,
                )
            }
            CopySelectionItem::PullRequestUrl(branch_name) => {
                fn get_review(
                    name: &str,
                    ctx: &Context,
                    cache_config: but_forge::CacheConfig,
                ) -> anyhow::Result<Option<String>> {
                    let review_map = crate::command::legacy::forge::review::get_review_map(
                        ctx,
                        Some(cache_config),
                    )?;
                    let url = review_map
                        .get(name)
                        .and_then(|reviews| reviews.first())
                        .map(|review| review.html_url.clone());
                    Ok(url)
                }

                let name = branch_name.shorten().to_str_lossy();
                let url = get_review(&name, ctx, but_forge::CacheConfig::CacheOnly)
                    .transpose()
                    .or_else(|| get_review(&name, ctx, but_forge::CacheConfig::NoCache).transpose())
                    .transpose()?
                    .context("No pull request URL found")?;

                Ok(url)
            }
            CopySelectionItem::ShortId(id) => Ok(id.to_owned()),
            CopySelectionItem::HunkDiff(uncommitted_hunk_or_file) => {
                uncommitted_hunk_or_file_to_diff(ctx, uncommitted_hunk_or_file)
            }
            CopySelectionItem::FilePath(path) => Ok(path.to_owned()),
        }
    }
}

impl FuzzyPickerItem for CopySelectionItem {
    fn columns(&self, searchable: SearchableToken) -> impl IntoIterator<Item = Col<'_>> {
        [Col {
            text: self.as_str().into(),
            searchable: Some(searchable),
        }]
    }

    fn style(&self, theme: &'static Theme) -> Style {
        theme.default
    }
}

// TODO(david): this is likely duplicated elswhere in the but crate or should otherwise be in a
// shared place
fn uncommitted_hunk_or_file_to_diff(
    ctx: &Context,
    uncommitted: &UncommittedHunkOrFile,
) -> anyhow::Result<String> {
    let repo = ctx.repo.get()?;
    let worktree_changes = but_api::diff::changes_in_worktree(ctx)?;
    let assignments: Vec<_> = worktree_changes
        .assignments
        .into_iter()
        .filter(|assignment| uncommitted_hunk_matches_selection(assignment, uncommitted))
        .collect();

    let mut diff = String::new();
    let mut whole_file_diff_paths = BTreeSet::new();
    for assignment in assignments {
        if assignment.hunk_header.is_some() {
            diff.push_str(&hunk_assignment_to_diff(&assignment));
        } else {
            whole_file_diff_paths.insert(assignment.path_bytes);
        }
    }

    let whole_file_changes = worktree_changes
        .worktree_changes
        .changes
        .into_iter()
        .filter(|change| whole_file_diff_paths.contains(&change.path_bytes))
        .map(Into::into)
        .collect();
    diff.push_str(&tree_changes_to_diff(
        whole_file_changes,
        &repo,
        ctx.settings.context_lines,
    )?);

    Ok(diff)
}

fn uncommitted_hunk_matches_selection(
    hunk_assignment: &HunkAssignment,
    uncommitted: &UncommittedHunkOrFile,
) -> bool {
    let selected_hunk = uncommitted.hunk_assignments.first();

    if uncommitted.is_entire_file {
        hunk_assignment.path_bytes == selected_hunk.path_bytes
            && hunk_assignment.stack_id == selected_hunk.stack_id
    } else {
        hunk_assignment == selected_hunk && hunk_assignment.stack_id == selected_hunk.stack_id
    }
}

fn hunk_assignment_to_diff(assignment: &HunkAssignment) -> String {
    let path = assignment.path_bytes.to_str_lossy();
    let mut diff = format!("diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n");
    if let Some(hunk_diff) = &assignment.diff {
        diff.push_str(&hunk_diff.to_str_lossy());
        if !diff.ends_with('\n') {
            diff.push('\n');
        }
    }
    diff
}

fn lowercase_first_letter(s: &str) -> Cow<'_, str> {
    let Some(first) = s.chars().next() else {
        return Cow::Borrowed(s);
    };

    let lowercase_first = first.to_lowercase().to_string();
    if lowercase_first == first.to_string() {
        return Cow::Borrowed(s);
    }

    let mut lowercased = lowercase_first;
    lowercased.push_str(&s[first.len_utf8()..]);
    Cow::Owned(lowercased)
}

fn commit(repo: &gix::Repository, commit_id: gix::ObjectId) -> anyhow::Result<CommitOwned> {
    Ok(but_core::Commit::from_id(commit_id.attach(repo))?.detach())
}

fn commit_details(
    repo: &gix::Repository,
    commit_id: gix::ObjectId,
) -> anyhow::Result<CommitDetails> {
    CommitDetails::from_commit_id(commit_id.attach(repo), false)
}

fn tree_changes_to_diff(
    tree_changes: Vec<TreeChange>,
    repo: &gix::Repository,
    context_lines: u32,
) -> anyhow::Result<String> {
    tree_changes
        .into_iter()
        .map(|change| change.unified_diff(repo, context_lines))
        .filter_map(|diff| diff.transpose())
        .try_fold(String::new(), |mut diff, line| -> anyhow::Result<_> {
            diff.push_str(&line?.to_str_lossy());
            Ok(diff)
        })
}
