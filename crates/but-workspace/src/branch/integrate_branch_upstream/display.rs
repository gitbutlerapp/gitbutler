//! Display-oriented types and helpers for branch upstream integration.

use std::{collections::HashMap, fmt};

use anyhow::Result;
use bstr::ByteSlice;
use but_core::commit::Headers;

use crate::{divergence::TargetCommitRelation, ui::Author};

/// A single commit row in the divergence display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationDivergenceCommit {
    /// The commit shown in the graph row.
    pub id: gix::ObjectId,
    /// The first-line subject shown for the commit.
    pub subject: String,
    /// The explicit GitButler Change-Id stored in the commit headers, if present.
    pub change_id: Option<String>,
    /// Commit creation time in Epoch milliseconds.
    pub created_at: i128,
    /// The author of the commit.
    pub author: Author,
    /// Human-facing ref labels rendered inline on the commit row.
    pub refs: Vec<String>,
    /// How the commit relates to the configured target branch.
    pub target_relation: IntegrationDivergenceTargetRelation,
}

/// How a divergence commit relates to the configured target branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrationDivergenceTargetRelation {
    /// The commit is not present in the target branch.
    NotIntegrated,
    /// The exact commit is reachable from target branch history.
    HistoricallyIntegrated {
        /// The target branch commit that establishes the relation.
        target_commit_id: gix::ObjectId,
    },
}

impl From<TargetCommitRelation> for IntegrationDivergenceTargetRelation {
    fn from(value: TargetCommitRelation) -> Self {
        match value {
            TargetCommitRelation::NotIntegrated => Self::NotIntegrated,
            TargetCommitRelation::HistoricallyIntegrated { target_commit_id } => {
                Self::HistoricallyIntegrated { target_commit_id }
            }
        }
    }
}

/// Current branch/upstream divergence information for display purposes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationDivergenceDisplay {
    /// The local branch being integrated.
    pub branch_ref_name: gix::refs::FullName,
    /// The upstream branch this local branch integrates with.
    pub upstream_ref_name: gix::refs::FullName,
    /// Commits only reachable from the local branch tip down to the shared section.
    pub local_only: Vec<IntegrationDivergenceCommit>,
    /// Commits only reachable from the upstream branch tip down to the shared section.
    pub upstream_only: Vec<IntegrationDivergenceCommit>,
    /// The merge-base row shown once at the bottom.
    pub merge_base: IntegrationDivergenceCommit,
}

impl fmt::Display for IntegrationDivergenceDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for commit in &self.local_only {
            writeln!(f, "{}", graph_commit_string("* ", commit))?;
        }
        for commit in &self.upstream_only {
            let prefix = if self.local_only.is_empty() {
                "* "
            } else {
                "| * "
            };
            writeln!(f, "{}", graph_commit_string(prefix, commit))?;
        }
        if !self.local_only.is_empty() && !self.upstream_only.is_empty() {
            writeln!(f, "|/")?;
        }
        write!(f, "{}", graph_commit_string("* ", &self.merge_base))
    }
}

/// Build a display row for a single divergence commit.
///
/// `repo` provides commit decoding so the commit subject can be loaded.
///
/// `commit_id` is the commit that should be rendered in the divergence view.
///
/// `target_relation` describes how `commit_id` relates to the configured target
/// branch.
///
/// Returns the populated display row with subject text and target relation.
pub(super) fn divergence_commit(
    repo: &gix::Repository,
    commit_id: gix::ObjectId,
    target_relation: TargetCommitRelation,
) -> Result<IntegrationDivergenceCommit> {
    let commit_object = repo.find_commit(commit_id)?;
    let commit = commit_object.decode()?;
    let headers = Headers::try_from_commit_headers(|| commit.extra_headers());
    Ok(IntegrationDivergenceCommit {
        id: commit_id,
        subject: commit
            .message
            .lines()
            .next()
            .unwrap_or_default()
            .to_str_lossy()
            .into_owned(),
        change_id: headers
            .and_then(|headers| headers.change_id)
            .map(|change_id| change_id.to_string()),
        created_at: i128::from(commit.time()?.seconds) * 1000,
        author: commit.author()?.into(),
        refs: Vec::new(),
        target_relation: target_relation.into(),
    })
}

/// Attach a ref label to the matching displayed commit row.
///
/// `primary` is the primary list of display rows to search first, such as the
/// local-only or upstream-only side of the divergence.
///
/// `merge_base` is the shared merge-base row that should receive the label if
/// the labeled commit is not present in `primary`.
///
/// `id` is the optional commit id that should receive `label`.
///
/// `label` is the human-facing ref label to append when the target row is
/// found.
///
/// Returns `()` after applying the label in place when a matching commit row
/// exists.
pub(super) fn add_ref_label(
    primary: &mut [IntegrationDivergenceCommit],
    merge_base: &mut IntegrationDivergenceCommit,
    id: Option<gix::ObjectId>,
    label: String,
) {
    let Some(id) = id else {
        return;
    };
    if let Some(commit) = primary.iter_mut().find(|commit| commit.id == id) {
        if !commit.refs.contains(&label) {
            commit.refs.push(label);
        }
        return;
    }
    if merge_base.id == id && !merge_base.refs.contains(&label) {
        merge_base.refs.push(label);
    }
}

/// Look up the target-branch relation for a displayed commit.
///
/// `target_relations` is the precomputed map of commit ids to their target
/// branch relation.
///
/// `commit_id` is the commit whose relation should be retrieved.
///
/// Returns the stored relation for `commit_id`, or `NotIntegrated` when the
/// commit is absent from the map.
pub(super) fn relation_for(
    target_relations: &HashMap<gix::ObjectId, TargetCommitRelation>,
    commit_id: gix::ObjectId,
) -> TargetCommitRelation {
    target_relations
        .get(&commit_id)
        .copied()
        .unwrap_or(TargetCommitRelation::NotIntegrated)
}

fn graph_commit_string(prefix: &str, commit: &IntegrationDivergenceCommit) -> String {
    let refs = if commit.refs.is_empty() {
        String::new()
    } else {
        format!(" ({})", commit.refs.join(", "))
    };
    format!(
        "{prefix}{}{} {}",
        commit.id.to_hex_with_len(7),
        refs,
        commit.subject
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(hex: &str) -> gix::ObjectId {
        gix::ObjectId::from_hex(hex.as_bytes()).expect("valid object id")
    }

    #[test]
    fn divergence_display_renders_git_style_graph() {
        let display = IntegrationDivergenceDisplay {
            branch_ref_name: gix::refs::Category::LocalBranch
                .to_full_name("feature")
                .expect("valid local branch"),
            upstream_ref_name: gix::refs::Category::RemoteBranch
                .to_full_name("origin/feature")
                .expect("valid remote branch"),
            local_only: vec![IntegrationDivergenceCommit {
                id: oid("1111111111111111111111111111111111111111"),
                subject: "local tip".into(),
                change_id: None,
                created_at: 0,
                author: author(),
                refs: vec!["feature".into()],
                target_relation: IntegrationDivergenceTargetRelation::NotIntegrated,
            }],
            upstream_only: vec![IntegrationDivergenceCommit {
                id: oid("2222222222222222222222222222222222222222"),
                subject: "remote tip".into(),
                change_id: None,
                created_at: 0,
                author: author(),
                refs: vec!["origin/feature".into()],
                target_relation: IntegrationDivergenceTargetRelation::NotIntegrated,
            }],
            merge_base: IntegrationDivergenceCommit {
                id: oid("3333333333333333333333333333333333333333"),
                subject: "base".into(),
                change_id: None,
                created_at: 0,
                author: author(),
                refs: Vec::new(),
                target_relation: IntegrationDivergenceTargetRelation::NotIntegrated,
            },
        };

        // graph output should stay stable because the CLI and frontend consume it directly
        snapbox::assert_data_eq!(
            display.to_string(),
            snapbox::str![[r#"
* 1111111 (feature) local tip
| * 2222222 (origin/feature) remote tip
|/
* 3333333 base
"#]]
        );
    }

    fn gravatar_url_from_email(email: &str) -> url::Url {
        let gravatar_url = format!(
            "https://www.gravatar.com/avatar/{:x}?s=100&r=g&d=retro",
            md5::compute(email.to_lowercase())
        );
        url::Url::parse(gravatar_url.as_str()).unwrap()
    }

    fn author() -> Author {
        Author {
            name: "Author".into(),
            email: "author@example.com".into(),
            gravatar_url: gravatar_url_from_email("author@example.com"),
        }
    }
}
