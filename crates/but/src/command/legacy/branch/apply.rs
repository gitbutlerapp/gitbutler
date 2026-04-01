use std::{ops::Deref, str::FromStr};

use anyhow::bail;
use bstr::ByteSlice;
use but_ctx::Context;
use gitbutler_reference::{Refname, RemoteRefname};
use gix::reference::Category;

use crate::utils::OutputChannel;

/// Apply a branch to the workspace, and return the full ref name to it.
///
/// Look first in through the local references, then the remote references.
/// If no exact match is found, look for branches whose names contain the given string
/// and offer an interactive selection.
pub fn apply(ctx: &mut Context, branch_name: &str, out: &mut OutputChannel) -> anyhow::Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    let (reference, ref_name, remote_ref_name) = {
        let repo = &*ctx.repo.get()?;
        resolve_reference_to_apply(repo, branch_name, out)?
    };
    but_api::legacy::virtual_branches::create_virtual_branch_from_branch_with_perm(
        ctx,
        &ref_name,
        remote_ref_name,
        None,
        guard.write_permission(),
    )?;

    if let Some(out) = out.for_human() {
        let short_name = reference.name.shorten();
        let is_remote_reference = reference
            .name
            .category()
            .is_some_and(|c| c == Category::RemoteBranch);
        if is_remote_reference {
            writeln!(out, "Applied remote branch '{short_name}' to workspace")
        } else {
            writeln!(out, "Applied branch '{short_name}' to workspace")
        }?;
    } else if let Some(out) = out.for_shell() {
        writeln!(out, "{reference_name}", reference_name = reference.name)?;
    }

    if let Some(out) = out.for_json() {
        out.write_value(but_api::json::Reference::from(reference))?;
    }
    Ok(())
}

/// Resolve `branch_name` to the detached reference and metadata needed to apply it.
fn resolve_reference_to_apply(
    repo: &gix::Repository,
    branch_name: &str,
    out: &mut OutputChannel,
) -> anyhow::Result<(gix::refs::Reference, Refname, Option<RemoteRefname>)> {
    if let Some(reference) = repo.try_find_reference(branch_name)? {
        return local_reference_to_apply(reference);
    }

    if let Some((remote_ref_name, reference)) = find_remote_reference(repo, branch_name)? {
        return remote_reference_to_apply(remote_ref_name, reference);
    }

    // No exact match - look for branches whose names contain the given string.
    let mut partial = find_partial_matches(repo, branch_name)?;
    if partial.is_empty() {
        // NOTE: this is aligned with the 'modern' version for now which doesn't handle this specifically.
        //       Once there is only one impl, this can be adjusted more easily.
        bail!("The reference '{branch_name}' did not exist");
    }

    // Sort by last commit date, most recent first.
    partial.sort_by_key(|branch| std::cmp::Reverse(branch.timestamp()));

    match select_partial_match(partial, branch_name, out)? {
        PartialMatch::Local {
            display, full_name, ..
        } => {
            let reference = repo
                .try_find_reference(&full_name)?
                .ok_or_else(|| anyhow::anyhow!("Branch '{display}' not found"))?;
            local_reference_to_apply(reference)
        }
        PartialMatch::Remote {
            display, full_name, ..
        } => {
            let reference = repo
                .try_find_reference(&full_name)?
                .ok_or_else(|| anyhow::anyhow!("Branch '{display}' not found"))?;
            remote_reference_to_apply(RemoteRefname::from_str(&full_name.to_string())?, reference)
        }
    }
}

/// Convert a local reference into the detached reference and metadata needed to apply it.
fn local_reference_to_apply(
    reference: gix::Reference<'_>,
) -> anyhow::Result<(gix::refs::Reference, Refname, Option<RemoteRefname>)> {
    let ref_name = Refname::from_str(&reference.name().to_string())?;
    let remote_ref_name = reference
        .remote_ref_name(gix::remote::Direction::Push)
        .transpose()?
        .as_deref()
        .and_then(|ref_name| RemoteRefname::from_str(&ref_name.to_string()).ok());
    Ok((reference.detach(), ref_name, remote_ref_name))
}

/// Convert a remote-tracking reference into the detached reference and metadata needed to apply it.
fn remote_reference_to_apply(
    remote_ref_name: RemoteRefname,
    reference: gix::Reference<'_>,
) -> anyhow::Result<(gix::refs::Reference, Refname, Option<RemoteRefname>)> {
    let ref_name = Refname::from_str(&remote_ref_name.to_string())?;
    Ok((reference.detach(), ref_name, Some(remote_ref_name)))
}

enum PartialMatch {
    Local {
        display: String,
        full_name: gix::refs::FullName,
        timestamp: i64,
    },
    Remote {
        display: String,
        full_name: gix::refs::FullName,
        timestamp: i64,
    },
}

impl PartialMatch {
    fn display(&self) -> &str {
        match self {
            Self::Local { display, .. } | Self::Remote { display, .. } => display,
        }
    }

    fn timestamp(&self) -> i64 {
        match self {
            Self::Local { timestamp, .. } | Self::Remote { timestamp, .. } => *timestamp,
        }
    }
}

/// Find all local and remote branches whose names contain `pattern` (case-insensitive).
fn find_partial_matches(
    repo: &gix::Repository,
    pattern: &str,
) -> anyhow::Result<Vec<PartialMatch>> {
    fn reference_timestamp(reference: &mut gix::Reference<'_>) -> i64 {
        reference
            .peel_to_commit()
            .ok()
            .and_then(|commit| commit.time().ok())
            .map(|time| time.seconds)
            .unwrap_or(0)
    }

    let pattern_lower = pattern.to_lowercase();
    let mut matches = Vec::new();

    for mut reference in repo.references()?.all()?.filter_map(Result::ok) {
        let full_name = reference.name().to_owned();
        let Some((category, short_name)) = full_name.category_and_short_name() else {
            continue;
        };
        let display = short_name.to_string();

        match category {
            Category::LocalBranch => {
                if !display.to_lowercase().contains(&pattern_lower) {
                    continue;
                }
                matches.push(PartialMatch::Local {
                    display,
                    full_name,
                    timestamp: reference_timestamp(&mut reference),
                });
            }
            Category::RemoteBranch => {
                // Skip symbolic HEAD refs like "origin/HEAD"
                if display.ends_with("/HEAD") {
                    continue;
                }
                if !display.to_lowercase().contains(&pattern_lower) {
                    continue;
                }
                matches.push(PartialMatch::Remote {
                    display,
                    full_name,
                    timestamp: reference_timestamp(&mut reference),
                });
            }
            _ => {}
        }
    }

    Ok(matches)
}

/// Present an interactive selection prompt (or a non-interactive error) for partial branch matches.
fn select_partial_match(
    mut matches: Vec<PartialMatch>,
    pattern: &str,
    out: &mut OutputChannel,
) -> anyhow::Result<PartialMatch> {
    if !out.can_prompt() {
        if matches.len() == 1 {
            return Ok(matches.remove(0));
        }
        let options = matches
            .iter()
            .map(|m| format!("  {}", m.display()))
            .collect::<Vec<_>>()
            .join("\n");
        bail!("'{pattern}' didn't match exactly. Possible matches:\n{options}");
    }

    use cli_prompts::DisplayPrompt;

    let displays: Vec<String> = matches.iter().map(|m| m.display().to_string()).collect();
    let prompt = cli_prompts::prompts::Selection::new(
        &format!("'{pattern}' didn't match exactly. Which branch did you mean?"),
        displays.iter().cloned(),
    );
    let selected = prompt
        .display()
        .map_err(|e| anyhow::anyhow!("Selection aborted: {e:?}"))?;
    let idx = matches
        .iter()
        .position(|m| m.display() == selected)
        .ok_or_else(|| anyhow::anyhow!("Selected branch not found"))?;
    Ok(matches.remove(idx))
}

fn find_remote_reference<'repo>(
    repo: &'repo gix::Repository,
    branch_name: &str,
) -> anyhow::Result<Option<(RemoteRefname, gix::Reference<'repo>)>> {
    for remote in repo.remote_names().iter().map(|r| r.deref()) {
        let remote_ref_name = RemoteRefname::new(remote.to_str()?, branch_name);
        if let Some(reference) = repo.try_find_reference(&remote_ref_name.fullname())? {
            return Ok(Some((remote_ref_name, reference)));
        }
    }
    Ok(None)
}
