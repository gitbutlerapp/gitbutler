use std::{borrow::Cow, collections::BTreeSet};

use anyhow::{Context as _, Result};
use bstr::{BStr, ByteSlice};
use gix::refs::Category;

use crate::BranchIdentity;

// TODO(ST): replace the original with this one.
pub trait ReferenceExtGix {
    /// Produces a name by removing all prefixes, leaving only its actual name. All known
    /// `remotes` are needed to be able to strip remote names.
    ///
    /// Here are some examples:
    ///
    /// `refs/heads/my-branch` -> `my-branch`
    /// `refs/remotes/origin/my-branch` -> `my-branch`
    /// `refs/remotes/Byron/gitbutler/my-branch` -> `my-branch` (where the remote is `Byron/gitbutler`)
    fn identity(&self, remotes: &BTreeSet<Cow<'_, BStr>>) -> Result<BranchIdentity>;
}

impl ReferenceExtGix for &gix::refs::FullNameRef {
    fn identity(&self, remotes: &BTreeSet<Cow<'_, BStr>>) -> Result<BranchIdentity> {
        let (category, shorthand_name) = self
            .category_and_short_name()
            .context("Branch could not be categorized")?;
        if !matches!(category, Category::RemoteBranch) {
            return Ok(shorthand_name.try_into()?);
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
                "Failed to cut remote name {longest_remote} off of shorthand name {shorthand_name}"
            ))?
            .into();

        Ok(shorthand_name.try_into()?)
    }
}
