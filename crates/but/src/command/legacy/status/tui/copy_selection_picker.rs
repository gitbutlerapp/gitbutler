use std::borrow::Cow;

use anyhow::Context as _;
use bstr::ByteSlice as _;
use but_core::{CommitOwned, TreeChange, diff::CommitDetails};
use but_ctx::Context;
use gix::{prelude::ObjectIdExt as _, refs::FullName};
use nonempty::NonEmpty;
use ratatui::style::Style;

use crate::{
    command::legacy::status::tui::{
        Col, FuzzyPicker, FuzzyPickerItem, Message, SearchableToken, ToastKind,
    },
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
    CommitSha(gix::ObjectId),
    ShortCommitSha(gix::ObjectId),
    CommitMessageTitle(gix::ObjectId),
    WholeCommitMessage(gix::ObjectId),
    CommitAuthor(gix::ObjectId),
    CommitDiff(gix::ObjectId),
    BranchName(FullName),
    BranchDiff(FullName),
    PullRequestUrl(FullName),
}

impl CopySelectionItem {
    fn as_str(&self) -> &'static str {
        match self {
            CopySelectionItem::CommitSha(_) => "Commit ID",
            CopySelectionItem::ShortCommitSha(_) => "Short commit ID",
            CopySelectionItem::CommitMessageTitle(_) => "Message title",
            CopySelectionItem::WholeCommitMessage(_) => "Whole message",
            CopySelectionItem::CommitAuthor(_) => "Author",
            CopySelectionItem::CommitDiff(_) | CopySelectionItem::BranchDiff(_) => "Diff",
            CopySelectionItem::BranchName(_) => "Branch name",
            CopySelectionItem::PullRequestUrl(_) => "Pull Request URL",
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
