//! Recovery receipts shared by composed and API workspace transactions.

use anyhow::Context as _;
use bstr::{BString, ByteSlice as _};
use but_core::worktree::{checkout, safe_checkout_from_head};
use gix::{
    ObjectId,
    prelude::ObjectIdExt as _,
    refs::{
        FullName, FullNameRef, Target,
        transaction::{PreviousValue, RefEdit},
    },
};
use std::collections::BTreeMap;

#[derive(Debug, Default)]
/// Receipts of completed reference and checkout writes, restored conditionally on failure.
pub struct GitRecovery {
    /// Reference edits returned by native transactions, including observed old values.
    pub committed: Vec<RefEdit>,
    /// Snapshots for checkouts this operation may complete.
    pub checkouts: Vec<CheckoutSnapshot>,
}

impl GitRecovery {
    /// Capture affected checkouts and record completed Git changes for a later rollback.
    pub fn materialize<'ws, 'db, 'conn>(
        &mut self,
        rebase: super::SuccessfulRebase<'ws, 'db, 'conn>,
        options: super::materialize::MaterializeOptions,
    ) -> anyhow::Result<super::MaterializeOutcome<'ws, 'db, 'conn>> {
        if !options.without_checkout && rebase.references_updated()? {
            let names = rebase
                .workspace
                .graph
                .worktree_tips
                .iter()
                .map(|tip| tip.name.clone())
                .collect::<Vec<_>>();
            self.capture_checkouts(rebase.repo(), &names)?;
        }
        let outcome =
            rebase.materialize_with_changes(options, &mut self.committed, &mut |repo, target| {
                self.checkouts
                    .iter_mut()
                    .find(|checkout| checkout.repo.index_path() == repo.index_path())
                    .context("BUG: each materialized worktree has a recovery snapshot")?
                    .record_checkout(target)
            });
        let heads = self.record_materialized_heads();
        match (outcome, heads) {
            (Err(err), Err(head_error)) => Err(err.context(format!(
                "Could not record materialized HEAD: {head_error:#}"
            ))),
            (Err(err), _) | (_, Err(err)) => Err(err),
            (Ok(outcome), Ok(())) => Ok(outcome),
        }
    }

    /// Remove a reference created earlier in this operation while recording the deletion.
    pub fn remove_eagerly_created_ref(
        &mut self,
        repo: &gix::Repository,
        ref_name: &FullNameRef,
    ) -> anyhow::Result<()> {
        let Some((None, Some(target))) = self.ref_changes().remove(ref_name) else {
            return Ok(());
        };
        self.committed.extend(repo.edit_reference(RefEdit::delete(
            ref_name.to_owned(),
            PreviousValue::MustExistAndMatch(target),
        ))?);
        Ok(())
    }

    fn ref_changes(&self) -> BTreeMap<FullName, (Option<Target>, Option<Target>)> {
        use gix::refs::transaction::{Change, RefLog};
        let mut changes = BTreeMap::new();
        for edit in &self.committed {
            let expected = match &edit.change {
                Change::Update { log, .. } if log.mode == RefLog::Only => continue,
                Change::Delete {
                    log: RefLog::Only, ..
                } => continue,
                Change::Update { expected, .. } | Change::Delete { expected, .. } => expected,
            };
            // Native transactions replace this constraint only if the ref actually existed.
            // ExistingMustMatch can remain on a successful creation of an absent reference.
            let previous = match expected {
                PreviousValue::MustExistAndMatch(target) => Some(target.clone()),
                _ => None,
            };
            let new = edit.change.new_value().map(Into::into);
            changes
                .entry(edit.name.clone())
                .and_modify(|(original, current)| {
                    // An independently changed ref breaks our chain. Only undo the newest
                    // continuous suffix, retaining the intervening writer's value.
                    if *current != previous {
                        *original = previous.clone();
                    }
                    *current = new.clone();
                })
                .or_insert((previous, new));
        }
        changes
    }

    /// Snapshot the main checkout and the named linked worktrees before mutation.
    pub fn capture_checkouts(
        &mut self,
        repo: &gix::Repository,
        worktree_names: &[BString],
    ) -> anyhow::Result<()> {
        self.checkouts
            .push(CheckoutSnapshot::capture(repo.clone())?);
        if !worktree_names.is_empty() {
            let proxies = repo.worktrees()?;
            for name in worktree_names {
                let proxy = proxies
                    .iter()
                    .find(|proxy| proxy.id().is_ok_and(|id| id == name.as_bstr()))
                    .with_context(|| format!("Visible worktree {name} no longer exists"))?;
                self.checkouts
                    .push(CheckoutSnapshot::capture(proxy.clone().into_repo()?)?);
            }
        }
        Ok(())
    }

    /// Record checkout HEADs after materialization for independent-change detection.
    pub fn record_materialized_heads(&mut self) -> anyhow::Result<()> {
        for checkout in &mut self.checkouts {
            if checkout.target.is_some() {
                checkout.materialized_head = Some((
                    checkout.repo.head_name()?,
                    checkout.repo.head()?.id().map(|id| id.detach()),
                ));
            }
        }
        Ok(())
    }

    /// Restore completed checkouts and refs without overwriting detected independent edits.
    pub fn rollback(&mut self, repo: &gix::Repository) -> anyhow::Result<()> {
        let mut errors = Vec::new();
        // Each completed checkout recorded its target before refs could fail. Use that
        // known base to retain independently added worktree changes during restoration.
        for checkout in self.checkouts.drain(..).rev() {
            if let Err(err) = checkout.restore() {
                errors.push(format!("{err:#}"));
            }
        }
        for (name, (previous, current)) in self.ref_changes() {
            if previous == current {
                continue;
            }
            let expected = match current {
                Some(target) => PreviousValue::MustExistAndMatch(target),
                None => PreviousValue::MustNotExist,
            };
            let edit = match previous {
                Some(target) => {
                    RefEdit::update(name.clone(), target, expected, "rollback transaction")
                }
                None => RefEdit::delete(name.clone(), expected),
            };
            if let Err(err) = repo.edit_reference(edit) {
                errors.push(format!("Could not restore {name}: {err}"));
            }
        }
        self.committed.clear();
        anyhow::ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }

    /// Attach any safe-recovery failure to the original operation error.
    pub fn rollback_error(&mut self, repo: &gix::Repository, err: anyhow::Error) -> anyhow::Error {
        match self.rollback(repo) {
            Ok(()) => err,
            Err(recovery) => err.context(format!("Git transaction recovery failed: {recovery:#}")),
        }
    }
}

#[derive(Debug)]
/// Worktree contents and exact index bytes before a checkout.
pub struct CheckoutSnapshot {
    /// The repository whose worktree and index are being tracked.
    pub repo: gix::Repository,
    worktree: ObjectId,
    index: Option<Vec<u8>>,
    target: Option<ObjectId>,
    materialized_index: Option<Option<Vec<u8>>>,
    materialized_head: Option<(Option<FullName>, Option<ObjectId>)>,
}

impl CheckoutSnapshot {
    fn capture(repo: gix::Repository) -> anyhow::Result<Self> {
        let index = read_index_bytes(&repo)?;
        let head_tree = repo.head_tree_id_or_empty()?.detach();
        let changes = but_core::diff::worktree_changes_no_renames(&repo)?;
        let mut selection = changes
            .changes
            .iter()
            .map(|change| change.path.clone())
            .collect::<std::collections::BTreeSet<_>>();
        selection.extend(
            changes
                .index_changes
                .iter()
                .map(|change| change.location().to_owned()),
        );
        selection.extend(changes.index_conflicts.iter().map(|(path, _)| path.clone()));
        let snapshot = but_core::snapshot::create_tree(
            head_tree.attach(&repo),
            but_core::snapshot::create_tree::State {
                changes,
                selection,
                head: false,
            },
        )?;
        Ok(Self {
            repo,
            worktree: snapshot.worktree.unwrap_or(head_tree),
            index,
            target: None,
            materialized_index: None,
            materialized_head: None,
        })
    }

    /// Record the completed checkout target and resulting index bytes.
    pub fn record_checkout(&mut self, target: ObjectId) -> anyhow::Result<()> {
        self.target = Some(target);
        self.materialized_index = None;
        self.materialized_index = Some(read_index_bytes(&self.repo)?);
        Ok(())
    }

    fn restore(self) -> anyhow::Result<()> {
        use std::io::Write;
        let Some(target) = self.target else {
            return Ok(());
        };
        let materialized_index = self
            .materialized_index
            .context("Could not capture the completed checkout index for safe recovery")?;
        let materialized_head = self
            .materialized_head
            .context("Could not capture HEAD after materialization for safe recovery")?;
        anyhow::ensure!(
            (
                self.repo.head_name()?,
                self.repo.head()?.id().map(|id| id.detach())
            ) == materialized_head,
            "HEAD changed independently in {}; leaving its worktree intact",
            self.repo.git_dir().display()
        );
        let mut index_lock = gix::lock::File::acquire_to_update_resource(
            self.repo.index_path(),
            gix::lock::acquire::Fail::Immediately,
            None,
            0,
        )?;
        anyhow::ensure!(
            read_index_bytes(&self.repo)? == materialized_index,
            "Index changed independently in {}; leaving its staging intact",
            self.repo.git_dir().display()
        );
        // Keep ownership of the index until both files and staging are restored.
        safe_checkout_from_head(
            self.worktree,
            &self.repo,
            checkout::Options {
                skip_head_update: true,
                skip_index_update: true,
                merge_base_override: Some(target),
                ..Default::default()
            },
        )
        .with_context(|| {
            format!(
                "Could not restore worktree {}",
                self.repo.git_dir().display()
            )
        })?;
        if let Some(index) = self.index {
            index_lock.write_all(&index)?;
            index_lock.commit()?;
        } else if materialized_index.is_some() {
            std::fs::remove_file(self.repo.index_path())?;
        }
        Ok(())
    }
}

fn read_index_bytes(repo: &gix::Repository) -> anyhow::Result<Option<Vec<u8>>> {
    match std::fs::read(repo.index_path()) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}
