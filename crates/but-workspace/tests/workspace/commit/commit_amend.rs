use anyhow::Result;
use but_core::DiffSpec;
use but_rebase::graph_rebase::{Editor, LookupStep as _};
use but_testsupport::git_status;
use but_workspace::commit::commit_amend;

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack_with_segments,
    named_writable_scenario_with_description_and_graph as writable_scenario,
};

fn worktree_changes_as_specs(repo: &gix::Repository) -> Result<Vec<DiffSpec>> {
    Ok(but_core::diff::worktree_changes(repo)?
        .changes
        .into_iter()
        .map(DiffSpec::from)
        .collect())
}

/// Build DiffSpecs with populated hunk_headers, matching how the production
/// UI/CLI sends them. This is important because the production path always
/// includes hunk headers even when all hunks of a file are selected.
fn worktree_changes_as_specs_with_hunks(
    repo: &gix::Repository,
    context_lines: u32,
) -> Result<Vec<DiffSpec>> {
    let changes = but_core::diff::worktree_changes(repo)?;
    let mut specs = Vec::new();
    for change in &changes.changes {
        let mut spec = DiffSpec::from(change);
        if let Some(but_core::UnifiedPatch::Patch { hunks, .. }) =
            change.unified_patch(repo, context_lines)?
        {
            spec.hunk_headers = hunks.iter().map(but_core::HunkHeader::from).collect();
        }
        specs.push(spec);
    }
    Ok(specs)
}

#[test]
fn amend_commit_smoke_test() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    let two_id = repo.rev_parse_single("two")?.detach();
    std::fs::write(
        repo.workdir_path("amended.txt").expect("non-bare"),
        "amended\n",
    )?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    let outcome = commit_amend(editor, two_id, worktree_changes_as_specs(&repo)?, 0)?;

    assert!(outcome.rejected_specs.is_empty());
    let selector = outcome.commit_selector.expect("selector exists");
    let materialized = outcome.rebase.materialize()?;
    let rewritten_id = materialized.lookup_pick(selector)?;

    let rewritten_commit = repo.find_commit(rewritten_id)?;
    assert_eq!(rewritten_commit.message_raw()?, "commit two\n");
    let spec = format!("{rewritten_id}:amended.txt");
    let object_with_path = repo.rev_parse_single(spec.as_str())?;
    assert_eq!(object_with_path.object()?.kind, gix::objs::Kind::Blob);

    Ok(())
}

/// Amending uncommitted changes into an earlier commit when a later commit
/// also touches the same file should leave no uncommitted changes afterwards.
///
/// Scenario:
///   - "save 1" creates test.txt with 3 lines
///   - "partial 1" adds line 1.1 (partial commit)
///   - Uncommitted: adds line 1.2
///   - Amend line 1.2 into "save 1"
///
/// After amend, "partial 1" will conflict (rebased onto new "save 1"),
/// but there should be no remaining uncommitted changes.
#[test]
fn amend_into_earlier_commit_leaves_no_uncommitted_changes() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("amend-with-partial-commit", |_| {})?;

    // Find the "save 1" commit (first commit on the stack, parent of "partial 1")
    let partial_1_id = repo.rev_parse_single("stack-1")?.detach();
    let partial_1_commit = repo.find_commit(partial_1_id)?;
    let save_1_id = partial_1_commit
        .parent_ids()
        .next()
        .expect("has parent")
        .detach();

    // Verify initial state: there should be uncommitted changes (line 1.2)
    let status_before = git_status(&repo)?;
    assert!(
        status_before.contains("test.txt"),
        "should have uncommitted changes to test.txt before amend, got: {status_before}"
    );

    let context_lines = 0;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let outcome = commit_amend(
        editor,
        save_1_id,
        worktree_changes_as_specs_with_hunks(&repo, context_lines)?,
        context_lines,
    )?;

    assert!(outcome.rejected_specs.is_empty());
    let _selector = outcome.commit_selector.expect("amend selector exists");
    let _materialized = outcome.rebase.materialize()?;

    // No uncommitted changes should remain: the change was amended into
    // "save 1", so it must not persist as an uncommitted worktree change.
    let status_after = git_status(&repo)?;
    assert_eq!(
        status_after, "",
        "expected no uncommitted changes after amending, but got:\n{status_after}"
    );

    Ok(())
}

/// Amending a modified file into one stack must not discard uncommitted
/// deletions on a different stack.
///
/// Scenario (two independent branches A and B in the workspace):
///   - Branch A: adds a-file.txt
///   - Branch B: adds b-file.txt
///   - Uncommitted: a-file.txt modified, b-file.txt deleted
///   - Amend only a-file.txt into A's commit
///
/// After amend, b-file.txt must still appear as a deleted uncommitted change.
#[test]
fn amend_with_two_stacks_preserves_uncommitted_deletions() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("amend-two-stacks-with-deletions", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        })?;

    let workdir = repo.workdir().expect("non-bare repo");

    // Verify initial state: a-file.txt modified, b-file.txt deleted
    let status_before = git_status(&repo)?;
    assert!(
        status_before.contains("a-file.txt"),
        "should have uncommitted changes to a-file.txt before amend, got: {status_before}"
    );
    assert!(
        status_before.contains("b-file.txt"),
        "should have uncommitted deletion of b-file.txt before amend, got: {status_before}"
    );
    assert!(
        !workdir.join("b-file.txt").exists(),
        "b-file.txt should be deleted on disk"
    );

    // Build DiffSpecs for only a-file.txt (the file we want to amend)
    let all_changes = but_core::diff::worktree_changes(&repo)?;
    let a_file_specs: Vec<DiffSpec> = all_changes
        .changes
        .iter()
        .filter(|c| c.path == "a-file.txt")
        .map(DiffSpec::from)
        .collect();
    assert_eq!(
        a_file_specs.len(),
        1,
        "should have exactly one spec for a-file.txt"
    );

    // Find the commit on branch A
    let a_commit_id = repo.rev_parse_single("A")?.detach();

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let outcome = commit_amend(editor, a_commit_id, a_file_specs, 0)?;

    assert!(outcome.rejected_specs.is_empty());
    let _materialized = outcome.rebase.materialize()?;

    // After amend: a-file.txt should no longer be modified (it was amended)
    // but b-file.txt should STILL be deleted (uncommitted deletion preserved)
    assert!(
        !workdir.join("b-file.txt").exists(),
        "b-file.txt should still be deleted on disk after amend"
    );

    let status_after = git_status(&repo)?;
    assert!(
        !status_after.contains("a-file.txt"),
        "a-file.txt should no longer appear as modified after amend, got:\n{status_after}"
    );
    assert!(
        status_after.contains("b-file.txt"),
        "b-file.txt should still appear as a deleted file after amend, got:\n{status_after}"
    );

    Ok(())
}

/// Amending uncommitted changes of a linked worktree into arbitrary commits via
/// [`commit_amend_from_worktree`].
///
/// The `worktree-amend` fixture has `main` checked out with commits
/// `base -> M1`, plus a linked worktree `wt` on branch `feat`
/// (`base -> F1`, adding `a-file`) with two uncommitted changes:
/// a tracked modification of `a-file` and an untracked `new-file`.
mod from_worktree {
    use anyhow::Result;
    use but_core::DiffSpec;
    use but_graph::Graph;
    use but_meta::VirtualBranchesTomlMetadata;
    use but_rebase::graph_rebase::{Editor, GraphEditorOptions, LookupStep as _};
    use but_testsupport::git_status_at_dir;
    use but_workspace::{commit::commit_amend_from_worktree, worktrees::open_worktree_repo};

    use crate::utils::writable_scenario_slow;

    /// The metadata is wrapped so its backing file is never written on drop.
    fn scenario() -> (
        gix::Repository,
        but_testsupport::gix_testtools::tempfile::TempDir,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    ) {
        let (repo, tmp) = writable_scenario_slow("worktree-amend");
        let meta = VirtualBranchesTomlMetadata::from_path(
            repo.path().join("should-never-be-written.toml"),
        )
        .expect("in-memory metadata handle always opens");
        (repo, tmp, std::mem::ManuallyDrop::new(meta))
    }

    /// Build the graph over `repo` with `feat` (checked out in the linked worktree
    /// named `wt`) seeded as a worktree tip, mirroring what `but-ctx` does with the
    /// `worktreeManipulation` feature flag enabled.
    fn graph_with_worktree_tip(
        repo: &gix::Repository,
        meta: &impl but_core::RefMetadata,
    ) -> Result<Graph> {
        let mut options = but_graph::init::Options::limited();
        options.worktree_tips = vec![but_graph::init::WorktreeTip {
            name: "wt".into(),
            ref_name: Some("refs/heads/feat".try_into()?),
            id: repo.find_reference("feat")?.peel_to_id()?.detach(),
        }];
        Graph::from_head(repo, meta, Default::default(), options)?.validated()
    }

    /// Force the worktree branch mutable, as the API layer does for all worktree
    /// tips - it is not reachable from `HEAD`, so its ref would silently never
    /// move otherwise.
    fn editor_options() -> Result<GraphEditorOptions> {
        Ok(GraphEditorOptions {
            extra_mutable_refs: vec!["refs/heads/feat".try_into()?],
            ..Default::default()
        })
    }

    fn whole_file_spec(path: &str) -> Vec<DiffSpec> {
        vec![DiffSpec {
            path: path.into(),
            ..Default::default()
        }]
    }

    fn paths(specs: &[DiffSpec]) -> Vec<String> {
        specs.iter().map(|spec| spec.path.to_string()).collect()
    }

    fn blob(repo: &gix::Repository, spec: &str) -> Result<String> {
        Ok(String::from_utf8(
            repo.rev_parse_single(spec)?.object()?.data.clone(),
        )?)
    }

    #[test]
    fn amend_into_the_worktrees_own_branch_head_moves_its_checkout() -> Result<()> {
        let (repo, _tmp, mut meta) = scenario();
        let wt_dir = repo.workdir().expect("non-bare").join("wt");

        let graph = graph_with_worktree_tip(&repo, &*meta)?;
        let mut ws = graph.into_workspace()?;
        let editor = Editor::create_with_opts(&mut ws, &mut *meta, &repo, &editor_options()?)?;

        let wt_repo = open_worktree_repo(&repo, "wt".into())?;
        let f1_id = repo.rev_parse_single("feat")?.detach();

        // Only the tracked modification - the untracked file must survive.
        let outcome = commit_amend_from_worktree(
            editor,
            f1_id,
            whole_file_spec("a-file"),
            0,
            &wt_repo,
            "wt".into(),
        )?;
        assert_eq!(
            outcome.rejected_specs.len(),
            0,
            "{:?}",
            outcome.rejected_specs
        );
        assert_eq!(paths(&outcome.consumed_specs), ["a-file"]);
        let selector = outcome.commit_selector.expect("a commit was amended");
        let materialized = outcome.rebase.materialize()?;
        let new_id = materialized.lookup_pick(selector)?;

        assert_eq!(
            repo.rev_parse_single("feat")?.detach(),
            new_id,
            "the worktree's branch moved to the amended commit"
        );
        assert_eq!(
            blob(&repo, &format!("{new_id}:a-file"))?,
            "one\ntwo\nthree\nfour\n",
            "the amended commit contains the worktree's uncommitted content"
        );

        let status = git_status_at_dir(&wt_dir)?;
        assert!(
            !status.contains("a-file"),
            "the merge-base override cancelled the consumed change during the worktree checkout: {status}"
        );
        assert!(
            status.contains("new-file"),
            "the dirty file that wasn't amended survives in the worktree: {status}"
        );
        Ok(())
    }

    #[test]
    fn amend_into_another_branch_leaves_the_worktree_tip_alone() -> Result<()> {
        let (repo, _tmp, mut meta) = scenario();
        let wt_dir = repo.workdir().expect("non-bare").join("wt");

        let graph = graph_with_worktree_tip(&repo, &*meta)?;
        let mut ws = graph.into_workspace()?;
        let editor = Editor::create_with_opts(&mut ws, &mut *meta, &repo, &editor_options()?)?;

        let wt_repo = open_worktree_repo(&repo, "wt".into())?;
        let f1_id = repo.rev_parse_single("feat")?.detach();
        let m1_id = repo.head_id()?.detach();

        // The untracked addition applies cleanly onto a commit outside the
        // worktree's history.
        let outcome = commit_amend_from_worktree(
            editor,
            m1_id,
            whole_file_spec("new-file"),
            0,
            &wt_repo,
            "wt".into(),
        )?;
        assert_eq!(
            outcome.rejected_specs.len(),
            0,
            "{:?}",
            outcome.rejected_specs
        );
        assert_eq!(paths(&outcome.consumed_specs), ["new-file"]);
        let selector = outcome.commit_selector.expect("a commit was amended");
        let consumed = outcome.consumed_specs.clone();
        let materialized = outcome.rebase.materialize()?;
        let new_id = materialized.lookup_pick(selector)?;

        assert_eq!(
            repo.rev_parse_single("feat")?.detach(),
            f1_id,
            "an untouched worktree branch stays byte-identical even though it was forced mutable"
        );
        assert_eq!(
            repo.head_id()?.detach(),
            new_id,
            "the target branch moved to the amended commit"
        );
        assert_eq!(blob(&repo, &format!("{new_id}:new-file"))?, "new\n");

        // The worktree's tip didn't move, so its checkout still holds the
        // now-committed change as an uncommitted duplicate...
        let wt_repo = open_worktree_repo(&repo, "wt".into())?;
        assert_eq!(wt_repo.head_id()?.detach(), f1_id);
        let status = git_status_at_dir(&wt_dir)?;
        assert!(
            status.contains("new-file"),
            "the consumed change is still on disk before the discard: {status}"
        );

        // ...which is exactly what the API-level fallback discards afterwards.
        let dropped = but_workspace::discard_workspace_changes(&wt_repo, consumed, 0)?;
        assert_eq!(dropped.len(), 0, "{dropped:?}");
        let status = git_status_at_dir(&wt_dir)?;
        assert!(
            !status.contains("new-file"),
            "the committed change was removed from the worktree: {status}"
        );
        assert!(
            status.contains("a-file"),
            "the dirty file that wasn't amended survives untouched: {status}"
        );
        Ok(())
    }

    #[test]
    fn amend_into_an_immutable_commit_fails_fast() -> Result<()> {
        let (repo, _tmp, mut meta) = scenario();

        let graph = graph_with_worktree_tip(&repo, &*meta)?;
        let mut ws = graph.into_workspace()?;
        // Unlike every other test here, no extra_mutable_refs: the worktree
        // branch commit is present in the graph but immutable. Amending into
        // it used to silently no-op (the amended commit was written but no ref
        // ever adopted it) - now it must fail fast, before anything is written.
        let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

        let f1_id = repo.rev_parse_single("feat")?.detach();
        let m1_id = repo.head_id()?.detach();

        let err = but_workspace::commit::commit_amend(editor, f1_id, whole_file_spec("a-file"), 0)
            .unwrap_err();
        assert!(
            err.to_string().contains("the commit is immutable"),
            "{err}"
        );

        assert_eq!(
            repo.rev_parse_single("feat")?.detach(),
            f1_id,
            "nothing moved"
        );
        assert_eq!(repo.head_id()?.detach(), m1_id, "nothing moved");
        Ok(())
    }

    #[test]
    fn amend_from_an_unknown_worktree_fails_without_moving_refs() -> Result<()> {
        let (repo, _tmp, mut meta) = scenario();

        let graph = graph_with_worktree_tip(&repo, &*meta)?;
        let mut ws = graph.into_workspace()?;
        let editor = Editor::create_with_opts(&mut ws, &mut *meta, &repo, &editor_options()?)?;

        let wt_repo = open_worktree_repo(&repo, "wt".into())?;
        let f1_id = repo.rev_parse_single("feat")?.detach();
        let m1_id = repo.head_id()?.detach();

        let err = commit_amend_from_worktree(
            editor,
            f1_id,
            whole_file_spec("a-file"),
            0,
            &wt_repo,
            "not-a-worktree".into(),
        )
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("No checkout is recorded for a linked worktree named not-a-worktree"),
            "{err}"
        );

        assert_eq!(
            repo.rev_parse_single("feat")?.detach(),
            f1_id,
            "nothing moved"
        );
        assert_eq!(repo.head_id()?.detach(), m1_id, "nothing moved");
        Ok(())
    }
}
