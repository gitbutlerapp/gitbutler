use anyhow::{Context, Result};
use bstr::{BStr, ByteSlice};
use gix::refs::Category;
use itertools::Itertools;
use std::borrow::Cow;
use std::collections::BTreeSet;

pub trait ReferenceExt {
    /// Fetches a branches name without the remote name attached
    ///
    /// refs/heads/my-branch -> my-branch
    /// refs/remotes/origin/my-branch -> my-branch
    /// refs/remotes/Byron/gitbutler/my-branch -> my-branch (where the remote is Byron/gitbutler)
    ///
    /// An ideal implementation wouldn't require us to list all the references,
    /// but there doesn't seem to be a libgit2 solution to this.
    fn given_name(&self, remotes: &git2::string_array::StringArray) -> Result<String>;
}

// TODO(ST): replace the original with this one.
pub trait ReferenceExtGix {
    /// Fetches a branches name without the remote name attached
    ///
    /// refs/heads/my-branch -> my-branch
    /// refs/remotes/origin/my-branch -> my-branch
    /// refs/remotes/Byron/gitbutler/my-branch -> my-branch (where the remote is Byron/gitbutler)
    ///
    /// An ideal implementation wouldn't require us to list all the references,
    /// but there doesn't seem to be a libgit2 solution to this.
    fn given_name(&self, remotes: &BTreeSet<Cow<'_, BStr>>) -> Result<String>;
}

impl<'repo> ReferenceExt for git2::Reference<'repo> {
    fn given_name(&self, remotes: &git2::string_array::StringArray) -> Result<String> {
        if self.is_remote() {
            let shorthand_name = self
                .shorthand()
                .ok_or(anyhow::anyhow!("Branch name was not utf-8"))?;

            let longest_remote = remotes
                .iter()
                .flatten()
                .sorted_by_key(|remote_name| -(remote_name.len() as i32))
                .find(|reference_name| shorthand_name.starts_with(reference_name))
                .ok_or(anyhow::anyhow!(
                    "Failed to find remote branch's corresponding remote"
                ))?;

            let shorthand_name = shorthand_name
                .strip_prefix(longest_remote)
                .and_then(|str| str.strip_prefix("/"))
                .ok_or(anyhow::anyhow!(
                    "Failed to cut remote name {} off of shorthand name {}",
                    longest_remote,
                    shorthand_name
                ))?;

            Ok(shorthand_name.to_string())
        } else {
            self.shorthand()
                .ok_or(anyhow::anyhow!("Branch name was not utf-8"))
                .map(String::from)
        }
    }
}

impl ReferenceExtGix for &gix::refs::FullNameRef {
    // TODO: make this `identity()`, and use `BranchIdentity` type.
    fn given_name(&self, remotes: &BTreeSet<Cow<'_, BStr>>) -> Result<String> {
        let (category, shorthand_name) = self
            .category_and_short_name()
            .context("Branch could not be categorized")?;
        if !matches!(category, Category::RemoteBranch) {
            return Ok(shorthand_name.to_string());
        }

        let longest_remote = remotes
            .iter()
            .rfind(|reference_name| shorthand_name.starts_with(reference_name))
            .ok_or(anyhow::anyhow!(
                "Failed to find remote branch's corresponding remote"
            ))?;

        let shorthand_name: &BStr = shorthand_name
            .strip_prefix(longest_remote.as_bytes())
            .and_then(|str| str.strip_prefix(b"/"))
            .ok_or(anyhow::anyhow!(
                "Failed to cut remote name {} off of shorthand name {}",
                longest_remote,
                shorthand_name
            ))?
            .into();

        // TODO(correctness): this should be `BString`, but can't change it yet.
        Ok(shorthand_name.to_string())
    }
}
