//! Functions for materializing a rebase
use anyhow::{Context, Result, bail};
use but_core::{
    ObjectStorageExt as _, RefMetadata,
    worktree::{checkout::Options, safe_checkout_from_head},
};
use gix::refs::{
    Target,
    transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog},
};
use std::time::Duration;

use crate::graph_rebase::{
    Checkout, MaterializeOutcome, Pick, Step, SuccessfulRebase, util::collect_ordered_parents,
};

impl<'ws, 'graph, M: RefMetadata> SuccessfulRebase<'ws, 'graph, M> {
    /// Materializes a history rewrite
    pub fn materialize(mut self) -> Result<MaterializeOutcome<'ws, 'graph, M>> {
        let repo = self.repo.clone();
        if let Some(memory) = self.repo.objects.take_object_memory() {
            memory.persist(self.repo)?;
        }

        let mut head_reference_update = None;
        let mut planned_checkouts = Vec::with_capacity(self.checkouts.len());
        for checkout in self.checkouts {
            match checkout {
                Checkout::Head {
                    selector,
                    merge_base_override,
                } => {
                    let selector = self.history.normalize_selector(selector)?;
                    let step = self.graph[selector.id].clone();

                    let (new_head, new_head_refname) = match step {
                        Step::None => bail!("Checkout selector is pointing to none"),
                        Step::Pick(Pick { id, .. }) => (id, None),
                        Step::Reference { refname, .. } => {
                            let parents = collect_ordered_parents(&self.graph, selector.id);
                            let parent_step_id =
                                parents.first().context("No first parent to reference")?;
                            let Step::Pick(Pick { id, .. }) = self.graph[*parent_step_id] else {
                                bail!("collect_ordered_parents should always return a commit pick");
                            };
                            (id, Some(refname))
                        }
                    };
                    head_reference_update = new_head_refname;
                    planned_checkouts.push((new_head, merge_base_override));
                }
            }
        }

        let mut ref_edits = self.ref_edits.clone();
        if let Some(refname) = head_reference_update
            && repo.head_name()?.as_ref() != Some(&refname)
        {
            let ref_short_name = refname.shorten().to_owned();
            ref_edits.push(RefEdit {
                change: Change::Update {
                    log: LogChange {
                        mode: RefLog::AndReference,
                        force_create_reflog: false,
                        message: gix::reference::log::message(
                            "safe checkout",
                            ref_short_name.as_ref(),
                            0,
                        ),
                    },
                    expected: PreviousValue::Any,
                    new: Target::Symbolic(refname),
                },
                name: "HEAD".try_into().expect("root refs are always valid"),
                deref: false,
            });
        }
        let perform_checkouts = || -> Result<()> {
            for (new_head, merge_base_override) in planned_checkouts {
                safe_checkout_from_head(
                    new_head,
                    &repo,
                    Options {
                        skip_head_update: true,
                        merge_base_override,
                        allow_conflicted_commit_checkout: true,
                    },
                )?;
            }
            Ok(())
        };
        if self.prepare_ref_edits_before_checkout {
            with_prepared_reference_transaction(&repo, ref_edits, perform_checkouts)?;
        } else {
            perform_checkouts()?;
            repo.edit_references(ref_edits)?;
        }

        let project_meta = self.workspace.graph.project_meta.clone();
        self.workspace
            .refresh_from_head(&repo, &*self.meta, project_meta)?;

        Ok(MaterializeOutcome {
            graph: self.graph,
            history: self.history,
            workspace: self.workspace,
            meta: self.meta,
        })
    }

    /// Materializes a rebase without performing a checkout.
    ///
    /// For the vast majority of operations you want to use
    /// [`Self::materialize`]. This is intended to be used in niche cases like
    /// `uncommit`.
    ///
    /// This has means that we don't "cherry pick" the uncommitted changes from
    /// the old head onto the new one.
    ///
    /// If I dropped a commit from the history,
    /// [`Self::materialize_without_checkout`] will now see those changes in
    /// your working directory.
    ///
    /// If I instead called [`Self::materialize`], the changes would instead be
    /// gone from disk.
    pub fn materialize_without_checkout(mut self) -> Result<MaterializeOutcome<'ws, 'graph, M>> {
        let repo = self.repo.clone();
        if let Some(memory) = self.repo.objects.take_object_memory() {
            memory.persist(self.repo)?;
        }

        repo.edit_references(self.ref_edits.clone())?;

        let project_meta = self.workspace.graph.project_meta.clone();
        self.workspace
            .refresh_from_head(&repo, &*self.meta, project_meta)?;

        Ok(MaterializeOutcome {
            graph: self.graph,
            history: self.history,
            workspace: self.workspace,
            meta: self.meta,
        })
    }
}

fn with_prepared_reference_transaction<T>(
    repo: &gix::Repository,
    edits: Vec<RefEdit>,
    effect: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let (files_timeout, packed_timeout) = reference_lock_timeouts(repo);
    let transaction = repo
        .refs
        .transaction()
        .prepare(edits, files_timeout, packed_timeout)?;
    let outcome = effect()?;
    transaction.commit(repo.committer().transpose()?)?;
    Ok(outcome)
}

fn reference_lock_timeouts(
    repo: &gix::Repository,
) -> (gix::lock::acquire::Fail, gix::lock::acquire::Fail) {
    fn read(
        repo: &gix::Repository,
        key: &'static gix::config::tree::keys::LockTimeout,
        default_ms: u64,
    ) -> gix::lock::acquire::Fail {
        let mut trusted = gix::config::section::is_trusted;
        repo.config_snapshot()
            .plumbing()
            .integer_filter(key, &mut trusted)
            .and_then(|value| key.try_into_lock_timeout(value).ok())
            .unwrap_or_else(|| Duration::from_millis(default_ms).into())
    }

    (
        read(repo, &gix::config::tree::Core::FILES_REF_LOCK_TIMEOUT, 100),
        read(repo, &gix::config::tree::Core::PACKED_REFS_TIMEOUT, 1000),
    )
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, anyhow};
    use gix::refs::{
        Target,
        transaction::{Change, LogChange, PreviousValue, RefEdit},
    };

    use super::{reference_lock_timeouts, with_prepared_reference_transaction};

    fn repo_with_ref() -> Result<(
        tempfile::TempDir,
        gix::Repository,
        gix::refs::FullName,
        gix::ObjectId,
        gix::ObjectId,
    )> {
        let dir = tempfile::tempdir()?;
        gix::init(dir.path())?;
        let repo = but_testsupport::open_repo(dir.path())?;
        let old = repo.write_blob(b"old")?.detach();
        let new = repo.write_blob(b"new")?.detach();
        let name = gix::refs::FullName::try_from("refs/heads/guarded")?;
        repo.reference(
            name.as_ref(),
            old,
            PreviousValue::MustNotExist,
            "create guarded ref",
        )?;
        Ok((dir, repo, name, old, new))
    }

    fn update(name: gix::refs::FullName, old: gix::ObjectId, new: gix::ObjectId) -> RefEdit {
        RefEdit {
            name,
            change: Change::Update {
                log: LogChange::default(),
                expected: PreviousValue::MustExistAndMatch(Target::Object(old)),
                new: Target::Object(new),
            },
            deref: false,
        }
    }

    #[test]
    fn prepared_reference_transaction_holds_the_ref_lock_during_the_effect() -> Result<()> {
        let (_dir, repo, name, old, new) = repo_with_ref()?;
        with_prepared_reference_transaction(&repo, vec![update(name.clone(), old, new)], || {
            let competing = repo.reference(
                name.as_ref(),
                repo.write_blob(b"competing")?.detach(),
                PreviousValue::ExistingMustMatch(old.into()),
                "competing update",
            );
            assert!(
                competing.is_err(),
                "prepared transaction holds the ref lock"
            );
            Ok(())
        })?;
        assert_eq!(
            repo.find_reference(name.as_ref())?.peel_to_id()?.detach(),
            new
        );
        Ok(())
    }

    #[test]
    fn failed_effect_drops_the_ref_lock_without_committing() -> Result<()> {
        let (_dir, repo, name, old, new) = repo_with_ref()?;
        let err = with_prepared_reference_transaction(
            &repo,
            vec![update(name.clone(), old, new)],
            || Err::<(), _>(anyhow!("injected effect failure")),
        )
        .expect_err("effect fails");
        assert!(err.to_string().contains("injected effect failure"));
        assert_eq!(
            repo.find_reference(name.as_ref())?.peel_to_id()?.detach(),
            old
        );
        repo.reference(
            name.as_ref(),
            new,
            PreviousValue::ExistingMustMatch(old.into()),
            "lock was released",
        )?;
        Ok(())
    }

    #[test]
    fn reference_lock_timeouts_match_gix_defaults() -> Result<()> {
        let (_dir, repo, _name, _old, _new) = repo_with_ref()?;
        let (files, packed) = reference_lock_timeouts(&repo);
        assert_eq!(
            files,
            std::time::Duration::from_millis(100).into(),
            "loose refs use gix's default timeout"
        );
        assert_eq!(
            packed,
            std::time::Duration::from_millis(1000).into(),
            "packed refs use gix's default timeout"
        );
        Ok(())
    }

    #[test]
    fn reference_lock_timeouts_honor_valid_overrides() -> Result<()> {
        let dir = tempfile::tempdir()?;
        gix::init(dir.path())?;
        let repo = gix::open_opts(
            dir.path(),
            but_testsupport::open_repo_config()?
                .config_overrides(["core.filesRefLockTimeout=0", "core.packedRefsTimeout=2500"]),
        )?;
        let (files, packed) = reference_lock_timeouts(&repo);
        assert_eq!(files, gix::lock::acquire::Fail::Immediately);
        assert_eq!(packed, std::time::Duration::from_millis(2500).into());

        let repo = gix::open_opts(
            dir.path(),
            but_testsupport::open_repo_config()?
                .config_overrides(["core.filesRefLockTimeout=-1", "core.packedRefsTimeout=-1"]),
        )?;
        let forever = std::time::Duration::from_secs(u64::MAX).into();
        assert_eq!(reference_lock_timeouts(&repo), (forever, forever));
        Ok(())
    }

    #[test]
    fn reference_lock_timeouts_ignore_invalid_or_untrusted_values() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let repo = gix::init(dir.path())?;
        let config_path = repo.git_dir().join("config");
        use std::io::Write as _;
        writeln!(
            std::fs::OpenOptions::new().append(true).open(config_path)?,
            "[core]\nfilesRefLockTimeout = 0\npackedRefsTimeout = 0"
        )?;

        let invalid = gix::open_opts(
            dir.path(),
            but_testsupport::open_repo_config()?.config_overrides([
                "core.filesRefLockTimeout=invalid",
                "core.packedRefsTimeout=invalid",
            ]),
        )?;
        let defaults = (
            std::time::Duration::from_millis(100).into(),
            std::time::Duration::from_millis(1000).into(),
        );
        assert_eq!(reference_lock_timeouts(&invalid), defaults);

        let untrusted = gix::open_opts(
            dir.path(),
            but_testsupport::open_repo_config()?.with(gix::sec::Trust::Reduced),
        )?;
        assert_eq!(reference_lock_timeouts(&untrusted), defaults);
        Ok(())
    }
}
