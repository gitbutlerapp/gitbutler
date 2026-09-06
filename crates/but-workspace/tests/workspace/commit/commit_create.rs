use anyhow::Result;
use but_core::DiffSpec;
use but_rebase::graph_rebase::{Editor, anchor::Anchor, mutate::InsertSide};
use but_workspace::commit::{ChangeSource, commit_create};

use crate::ref_info::with_workspace_commit::utils::named_writable_scenario_with_description_and_graph as writable_scenario;

fn worktree_changes_as_specs(repo: &gix::Repository) -> Result<Vec<DiffSpec>> {
    Ok(but_core::diff::worktree_changes(repo)?
        .changes
        .into_iter()
        .map(DiffSpec::from)
        .collect())
}

#[test]
fn commit_above_commit() -> Result<()> {
    let (_tmp, ws, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    let two_id = repo.rev_parse_single("two")?.detach();
    std::fs::write(
        repo.workdir_path("inserted-above-commit.txt")
            .expect("non-bare"),
        "inserted\n",
    )?;

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut _meta, &repo)?;
    let outcome = commit_create(
        editor,
        worktree_changes_as_specs(&repo)?,
        Anchor::Commit(two_id),
        InsertSide::Above,
        "insert above commit",
        0,
        ChangeSource::Head,
    )?;

    assert!(outcome.rejected_specs.is_empty());
    let handle = outcome.commit.expect("a handle for the new commit");
    let new_commit_id = outcome.rebase.id_of(handle)?;
    outcome.rebase.materialize()?;

    let new_commit = repo.find_commit(new_commit_id)?;
    assert_eq!(new_commit.message_raw()?, "insert above commit");
    assert_eq!(
        new_commit.parent_ids().next().expect("one parent").detach(),
        two_id,
        "new commit should be based on the target commit when inserted above"
    );
    let mut two_ref = repo.find_reference("two")?;
    assert_eq!(
        two_ref.peel_to_id()?.detach(),
        new_commit_id,
        "the two reference should now point to the inserted commit"
    );

    Ok(())
}

#[test]
fn commit_below_commit() -> Result<()> {
    let (_tmp, ws, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    let one_id = repo.rev_parse_single("one")?.detach();
    let two_id = repo.rev_parse_single("two")?.detach();
    std::fs::write(
        repo.workdir_path("inserted-below-commit.txt")
            .expect("non-bare"),
        "inserted\n",
    )?;

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut _meta, &repo)?;
    let outcome = commit_create(
        editor,
        worktree_changes_as_specs(&repo)?,
        Anchor::Commit(two_id),
        InsertSide::Below,
        "insert below commit",
        0,
        ChangeSource::Head,
    )?;

    assert!(outcome.rejected_specs.is_empty());
    let handle = outcome.commit.expect("a handle for the new commit");
    let new_commit_id = outcome.rebase.id_of(handle)?;
    outcome.rebase.materialize()?;

    let new_commit = repo.find_commit(new_commit_id)?;
    assert_eq!(new_commit.message_raw()?, "insert below commit");
    assert_eq!(
        new_commit.parent_ids().next().expect("one parent").detach(),
        one_id,
        "new commit should be based on the target's first parent when inserted below"
    );

    Ok(())
}

#[test]
fn commit_above_reference() -> Result<()> {
    let (_tmp, ws, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    let two_id = repo.rev_parse_single("two")?.detach();
    let reference = repo.find_reference("two")?;
    std::fs::write(
        repo.workdir_path("inserted-above-reference.txt")
            .expect("non-bare"),
        "inserted\n",
    )?;

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut _meta, &repo)?;
    let outcome = commit_create(
        editor,
        worktree_changes_as_specs(&repo)?,
        Anchor::Reference(reference.name().to_owned()),
        InsertSide::Above,
        "insert above reference",
        0,
        ChangeSource::Head,
    )?;

    assert!(outcome.rejected_specs.is_empty());
    let handle = outcome.commit.expect("a handle for the new commit");
    let new_commit_id = outcome.rebase.id_of(handle)?;
    outcome.rebase.materialize()?;

    let new_commit = repo.find_commit(new_commit_id)?;
    assert_eq!(new_commit.message_raw()?, "insert above reference");
    assert_eq!(
        new_commit.parent_ids().next().expect("one parent").detach(),
        two_id,
        "new commit should be based on the referenced commit"
    );
    let mut two_ref = repo.find_reference("two")?;
    assert_eq!(
        two_ref.peel_to_id()?.detach(),
        two_id,
        "when inserting above a reference, the reference keeps pointing to the original commit"
    );

    Ok(())
}

#[test]
fn commit_below_merge_commit_uses_first_parent() -> Result<()> {
    let (_tmp, ws, repo, mut _meta, _description) =
        writable_scenario("merge-with-two-branches-line-offset", |_| {})?;
    let merge_id = repo.rev_parse_single("HEAD")?.detach();
    let merge_commit = repo.find_commit(merge_id)?;
    let first_parent_id = merge_commit
        .parent_ids()
        .next()
        .expect("merge commit has parent")
        .detach();
    std::fs::write(
        repo.workdir_path("inserted-below-merge.txt")
            .expect("non-bare"),
        "inserted\n",
    )?;

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut _meta, &repo)?;
    let outcome = commit_create(
        editor,
        worktree_changes_as_specs(&repo)?,
        Anchor::Commit(merge_id),
        InsertSide::Below,
        "insert below merge",
        0,
        ChangeSource::Head,
    )?;

    assert!(outcome.rejected_specs.is_empty());
    let handle = outcome.commit.expect("a handle for the new commit");
    let new_commit_id = outcome.rebase.id_of(handle)?;
    outcome.rebase.materialize()?;

    let new_commit = repo.find_commit(new_commit_id)?;
    assert_eq!(new_commit.message_raw()?, "insert below merge");
    assert_eq!(
        new_commit
            .parent_ids()
            .next()
            .expect("has a parent")
            .detach(),
        first_parent_id,
        "for below merge commits, we base creation on first parent"
    );

    Ok(())
}

#[test]
fn commit_all_rejected_is_noop() -> Result<()> {
    let (_tmp, ws, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    let two_id = repo.rev_parse_single("two")?.detach();
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut _meta, &repo)?;

    let outcome = commit_create(
        editor,
        vec![DiffSpec {
            previous_path: None,
            path: "does-not-exist".into(),
            hunk_headers: vec![],
        }],
        Anchor::Commit(two_id),
        InsertSide::Above,
        "no-op commit",
        0,
        ChangeSource::Head,
    )?;

    assert!(
        outcome.commit.is_none(),
        "no handle if there is no new commit"
    );
    assert_eq!(
        outcome.rejected_specs.len(),
        1,
        "the invalid spec should be rejected"
    );
    assert_eq!(outcome.rejected_specs[0].1.path, "does-not-exist");

    Ok(())
}

#[test]
fn commit_below_the_empty_bottom_of_an_ad_hoc_stack_rebases_the_branches_above() -> Result<()> {
    use but_core::RefMetadata as _;
    let (_tmp, repo, _legacy_meta) =
        crate::ref_info::with_workspace_commit::utils::named_writable_scenario(
            "single-branch-with-3-commits",
        )?;
    let head = repo.head_id()?.detach();
    let c2 = repo.rev_parse_single("HEAD~1")?.detach();
    let c1 = repo.rev_parse_single("HEAD~2")?.detach();
    for (name, id) in [
        ("refs/heads/top", head),
        ("refs/heads/middle", c2),
        ("refs/heads/bottom", c1),
        ("refs/heads/main", c1),
        ("refs/remotes/origin/main", c1),
    ] {
        repo.reference(
            name,
            id,
            gix::refs::transaction::PreviousValue::Any,
            "probe",
        )?;
    }
    repo.edit_reference(gix::refs::transaction::RefEdit {
        change: gix::refs::transaction::Change::Update {
            log: Default::default(),
            expected: gix::refs::transaction::PreviousValue::Any,
            new: gix::refs::Target::Symbolic("refs/heads/top".try_into()?),
        },
        name: "HEAD".try_into()?,
        deref: false,
    })?;
    let mut meta = but_meta::BranchOrderMetadata::from_paths(
        repo.path().join("virtual-branches.toml"),
        repo.path(),
    )?;
    meta.set_branch_stack_order(&[
        "refs/heads/top".try_into()?,
        "refs/heads/middle".try_into()?,
        "refs/heads/bottom".try_into()?,
    ])?;
    let project_meta = but_core::ref_metadata::ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        ..Default::default()
    };
    let ws = but_graph::Workspace::from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
        but_graph::walk::Options::limited(),
    )?;
    let editor = Editor::for_workspace(&ws, &mut meta, &repo)?;
    // The transaction crate round-trips through a rebase before every operation.
    let editor = editor.rebase()?.into_editor();
    let outcome = commit_create(
        editor,
        Vec::new(),
        Anchor::Reference("refs/heads/bottom".try_into()?),
        InsertSide::Below,
        "on bottom",
        0,
        ChangeSource::Head,
    )?;
    let preview = but_workspace::workspace::overlayed_workspace(&ws, &outcome.rebase)?;
    // The new commit lands under `bottom`; `middle` and `top` rebase onto it, while `main`
    // (the target's local branch, also resting on the base) stays where it was.
    snapbox::assert_data_eq!(
        but_testsupport::graph_workspace(&preview).to_string(),
        snapbox::str![[r#"
⌂:top[🌳] <> ✓refs/remotes/origin/main on 3d57fc1
└── ≡:top[🌳] on 3d57fc1 {1}
    ├── :top[🌳]
    │   └── ·17a490f
    ├── :middle
    │   └── ·79125a3
    └── 📙:bottom
        └── ·b7e60aa

"#]]
    );
    Ok(())
}
