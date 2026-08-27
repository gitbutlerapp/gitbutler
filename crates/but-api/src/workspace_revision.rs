//! Checksum for the state that determines a legacy workspace projection.

use std::collections::HashMap;

use sha2::{Digest, Sha256};

const VERSION: &[u8] = b"workspace-v1";

/// Compute an opaque checksum of the inputs used to build `head_info`.
///
/// This deliberately excludes Gerrit and hashes symbolic branch targets by target name rather
/// than their peeled object ID. A symbolic branch targeting a tag or custom namespace can therefore
/// change its resolved commit without changing the revision.
pub fn compute(ctx: &but_ctx::Context) -> anyhow::Result<String> {
    let metadata = ctx.meta()?;
    let project = ctx.project_meta()?;
    let repo = ctx.repo.get()?;
    let mut db = ctx.db.get_cache_mut()?;
    let options = but_graph::init::Options {
        worktrees: ctx.settings.feature_flags.worktree_manipulation,
        ..but_graph::init::Options::limited()
    };
    let inputs =
        but_graph::capture_workspace_inputs(&repo, &metadata, &project, &mut db, &options)?;
    let reviews = but_forge::review_associations_by_head(&db)?;
    Ok(compute_from_snapshot(&inputs, &reviews))
}

pub(crate) fn compute_if_unchanged(
    source: Option<&but_graph::WorkspaceInputSnapshot>,
    current: &but_graph::WorkspaceInputSnapshot,
    reviews: &HashMap<String, but_forge::ReviewAssociation>,
) -> Option<String> {
    (source == Some(current)).then(|| compute_from_snapshot(current, reviews))
}

fn compute_from_snapshot(
    inputs: &but_graph::WorkspaceInputSnapshot,
    reviews: &HashMap<String, but_forge::ReviewAssociation>,
) -> String {
    let mut digest = CanonicalDigest::new();
    digest.field(b"graph-inputs", inputs.as_bytes());

    let mut reviews = reviews.iter().collect::<Vec<_>>();
    reviews.sort();
    for (head, (number, is_open, merged_head)) in reviews {
        digest.field(b"forge-head", head.as_bytes());
        digest.u64(b"forge-pr", *number as u64);
        digest.field(b"forge-open", &[*is_open as u8]);
        digest.optional_field(
            b"forge-merged-head",
            merged_head.as_deref().map(str::as_bytes),
        );
    }

    format!("workspace-v1:{:x}", digest.finish())
}

struct CanonicalDigest(Sha256);

impl CanonicalDigest {
    fn new() -> Self {
        let mut digest = Self(Sha256::new());
        digest.field(b"version", VERSION);
        digest
    }

    fn field(&mut self, name: &[u8], value: &[u8]) {
        self.0.update(name.len().to_be_bytes());
        self.0.update(name);
        self.0.update(value.len().to_be_bytes());
        self.0.update(value);
    }

    fn u64(&mut self, name: &[u8], value: u64) {
        self.field(name, &value.to_be_bytes());
    }

    fn optional_field(&mut self, name: &[u8], value: Option<&[u8]>) {
        self.field(name, &[value.is_some() as u8]);
        if let Some(value) = value {
            self.field(name, value);
        }
    }

    fn finish(self) -> impl std::fmt::LowerHex {
        self.0.finalize()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{CanonicalDigest, compute_from_snapshot, compute_if_unchanged};

    #[test]
    fn canonical_fields_do_not_alias_at_boundaries() {
        let mut left = CanonicalDigest::new();
        left.field(b"a", b"bc");
        let mut right = CanonicalDigest::new();
        right.field(b"ab", b"c");

        assert_ne!(
            format!("{:x}", left.finish()),
            format!("{:x}", right.finish())
        );
    }

    #[test]
    fn stale_workspace_is_not_paired_with_live_revision() -> anyhow::Result<()> {
        use but_testsupport::{CommandExt, git_at_dir, writable_scenario};

        let (repo, tmp) = writable_scenario("checkout-head-info");
        let ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        let meta = ctx.meta()?;
        let repo = ctx.repo.get()?;
        let options = but_graph::init::Options::limited();
        let source_inputs = {
            let mut db = ctx.db.get_cache_mut()?;
            let inputs = but_graph::capture_workspace_inputs(
                &repo,
                &meta,
                &ctx.project_meta()?,
                &mut db,
                &options,
            )?;
            but_graph::Graph::from_head(
                &repo,
                &meta,
                ctx.project_meta()?,
                &mut db,
                options.clone(),
            )?
            .into_workspace()?;
            inputs
        };

        git_at_dir(tmp.path())
            .args(["branch", "external-change"])
            .run();

        let current_inputs = {
            let mut db = ctx.db.get_cache_mut()?;
            but_graph::capture_workspace_inputs(
                &repo,
                &meta,
                &ctx.project_meta()?,
                &mut db,
                &options,
            )?
        };
        assert_eq!(
            compute_if_unchanged(Some(&source_inputs), &current_inputs, &Default::default()),
            None,
            "a stale projection must not claim the newer live repository revision"
        );
        Ok(())
    }

    #[test]
    fn exact_forge_associations_are_part_of_revision() -> anyhow::Result<()> {
        use but_testsupport::writable_scenario;

        let (repo, _tmp) = writable_scenario("checkout-head-info");
        let ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        let inputs = {
            let repo = ctx.repo.get()?;
            let mut db = ctx.db.get_cache_mut()?;
            but_graph::capture_workspace_inputs(
                &repo,
                &ctx.meta()?,
                &ctx.project_meta()?,
                &mut db,
                &but_graph::init::Options::limited(),
            )?
        };
        let without_review = compute_from_snapshot(&inputs, &Default::default());
        let with_open_review = compute_from_snapshot(
            &inputs,
            &HashMap::from([("feature".to_owned(), (42, true, None))]),
        );
        let with_merged_review = compute_from_snapshot(
            &inputs,
            &HashMap::from([("feature".to_owned(), (42, false, Some("abcdef".to_owned())))]),
        );

        assert_ne!(
            without_review, with_open_review,
            "the revision includes the exact forge map applied to the response"
        );
        assert_ne!(
            with_open_review, with_merged_review,
            "the revision includes forge state that can change the projection"
        );
        Ok(())
    }
}
