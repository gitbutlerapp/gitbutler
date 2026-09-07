use anyhow::Result;
use but_core::DiffSpec;
use but_rebase::graph_rebase::{Editor, LookupStep as _};
use but_testsupport::{git_status, visualize_commit_graph_all, visualize_tree};
use but_workspace::commit::{ChangeSource, commit_amend};
use gix::prelude::ObjectIdExt;
use snapbox::{IntoData, str};

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
    let (_tmp, graph, repo, mut meta, _description, mut db) =
        writable_scenario("reword-three-commits", |_| {})?;
    std::fs::write(
        repo.workdir_path("amended.txt").expect("non-bare"),
        "amended\n",
    )?;
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        str![[r#"
?? amended.txt

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        str![[r#"
* c9f444c (HEAD -> three) commit three
* 16fd221 (origin/two, two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    let two_id = repo.rev_parse_single("two")?.detach();
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    let outcome = commit_amend(
        editor,
        two_id,
        worktree_changes_as_specs(&repo)?,
        0,
        ChangeSource::Head,
    )?;
    assert!(
        outcome.rejected_specs.is_empty(),
        "{:?}",
        outcome.rejected_specs
    );
    let selector = outcome.commit_selector.expect("selector exists");
    let materialized = outcome.rebase.materialize(Default::default())?;
    let rewritten_id = materialized.lookup_pick(selector)?;

    snapbox::assert_data_eq!(git_status(&repo)?, str![[r#""#]]);
    // `two` keeps its message but is rewritten, and `three` is rebased onto it.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        str![[r#"
* 4b615e9 (HEAD -> three) commit three
* 6eb1f62 (two) commit two
| * 16fd221 (origin/two) commit two
|/  
* 8b426d0 (one) commit one

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_tree(rewritten_id.attach(&repo)).to_string(),
        str![[r#"
0f87971
├── .gitignore:100644:f4ec724 "/remote/\n"
├── amended.txt:100644:296cc37 "amended\n"
├── one.txt:100644:257cc56 "foo\n"
└── two.txt:100644:257cc56 "foo\n"

"#]]
        .raw()
    );
    Ok(())
}

/// Amending uncommitted changes into an earlier commit when a later commit
/// also touches the same file should leave no uncommitted changes afterwards.
///
/// Scenario:
///   - "save 1" creates test.txt with 3 lines
///   - "partial 1" adds line 1.1 (partial commit)
///   - Uncommitted: adds line 1.2 after an unchanged separator
///   - Amend line 1.2 into "save 1"
///
/// After amend, there should be no remaining uncommitted changes.
#[test]
fn amend_into_earlier_commit_leaves_no_uncommitted_changes() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description, mut db) =
        writable_scenario("amend-with-partial-commit", |_| {})?;
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        str![[r#"
 M test.txt

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        str![[r#"
* 946fc34 (HEAD -> stack-1) partial 1
* 641df85 save 1
* 84731fe (origin/main, main) base

"#]]
    );

    let save_1_id = repo.rev_parse_single("stack-1~1")?.detach();
    let context_lines = 0;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    let outcome = commit_amend(
        editor,
        save_1_id,
        worktree_changes_as_specs_with_hunks(&repo, context_lines)?,
        context_lines,
        ChangeSource::Head,
    )?;
    assert!(
        outcome.rejected_specs.is_empty(),
        "{:?}",
        outcome.rejected_specs
    );
    outcome.rebase.materialize(Default::default())?;

    // The change was amended into "save 1", so nothing is left in the worktree.
    snapbox::assert_data_eq!(git_status(&repo)?, str![[r#""#]]);
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        str![[r#"
* 2ab5372 (HEAD -> stack-1) partial 1
* af7d04e save 1
* 84731fe (origin/main, main) base

"#]]
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
    let (_tmp, graph, repo, mut meta, _description, mut db) =
        writable_scenario("amend-two-stacks-with-deletions", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        })?;
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        str![[r#"
 M a-file.txt
 D b-file.txt

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        str![[r#"
*   fbc3f1c (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/  
| * a57aa7a (A) add a-file
* | 0bc0f67 (B) add b-file
|/  
* 85efbe4 (origin/main, main) M

"#]]
    );

    let a_file_specs: Vec<DiffSpec> = but_core::diff::worktree_changes(&repo)?
        .changes
        .iter()
        .filter(|change| change.path == "a-file.txt")
        .map(DiffSpec::from)
        .collect();
    let a_commit_id = repo.rev_parse_single("A")?.detach();
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    let outcome = commit_amend(editor, a_commit_id, a_file_specs, 0, ChangeSource::Head)?;
    assert!(
        outcome.rejected_specs.is_empty(),
        "{:?}",
        outcome.rejected_specs
    );
    outcome.rebase.materialize(Default::default())?;

    // Only the amended modification is gone, the deletion on the other stack survives.
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        str![[r#"
 D b-file.txt

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        str![[r#"
*   eacbec3 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/  
| * 83b5708 (A) add a-file
* | 0bc0f67 (B) add b-file
|/  
* 85efbe4 (origin/main, main) M

"#]]
    );
    Ok(())
}

/// Committing and amending uncommitted changes of a *linked worktree*.
///
/// The `worktree-amend` fixture has `main` checked out with commits
/// `base -> M1`, plus a linked worktree `wt` on branch `feat` (`base -> F1`,
/// adding `a-file`) with two uncommitted changes: a tracked modification of
/// `a-file` and an untracked `new-file`. A second, detached worktree
/// `wt-detached` carries a commit `D1` that no branch points at.
mod from_worktree {
    use anyhow::Result;
    use but_core::DiffSpec;
    use but_graph::Graph;
    use but_meta::VirtualBranchesTomlMetadata;
    use but_rebase::graph_rebase::{Editor, mutate::InsertSide};
    use but_testsupport::{git_status_at_dir, visualize_commit_graph_all, visualize_tree};
    use but_workspace::{
        commit::{ChangeSource, commit_amend, commit_create},
        worktrees::open_worktree_repo,
    };
    use snapbox::{IntoData, str};

    use crate::utils::writable_scenario_slow;

    /// The metadata is wrapped so its backing file is never written on drop.
    fn scenario() -> (
        gix::Repository,
        but_testsupport::gix_testtools::tempfile::TempDir,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
        but_db::DbHandle,
    ) {
        let (repo, tmp) = writable_scenario_slow("worktree-amend");
        let meta = VirtualBranchesTomlMetadata::from_path(
            repo.path().join("should-never-be-written.toml"),
        )
        .expect("in-memory metadata handle always opens");
        let db = but_testsupport::project_db(&repo).expect("project database always opens");
        (repo, tmp, std::mem::ManuallyDrop::new(meta), db)
    }

    /// Build the graph over `repo` with both linked worktrees discovered from `db`,
    /// mirroring what `but-ctx` does with the `worktreeManipulation` flag enabled;
    /// adoption is marked as already run so they count as active.
    fn graph_with_worktree_tips(
        repo: &gix::Repository,
        meta: &impl but_core::RefMetadata,
        db: &mut but_db::DbHandle,
    ) -> Result<Graph> {
        db.worktree_meta_mut().mark_adopted()?;
        Graph::from_head(
            repo,
            meta,
            Default::default(),
            db,
            but_graph::init::Options {
                worktrees: true,
                ..but_graph::init::Options::limited()
            },
        )?
        .validated()
    }

    fn detached_tip(repo: &gix::Repository) -> Result<gix::ObjectId> {
        Ok(open_worktree_repo(repo, "wt-detached".into())?
            .head_id()?
            .detach())
    }

    fn whole_file_spec(path: &str) -> Vec<DiffSpec> {
        vec![DiffSpec {
            path: path.into(),
            ..Default::default()
        }]
    }

    fn tree_at(repo: &gix::Repository, spec: &str) -> Result<String> {
        Ok(visualize_tree(repo.rev_parse_single(spec)?).to_string())
    }

    #[test]
    fn amend_into_the_worktrees_own_branch_moves_its_checkout() -> Result<()> {
        let (repo, _tmp, mut meta, mut db) = scenario();
        let wt_dir = repo.workdir().expect("non-bare").join("wt");
        let graph = graph_with_worktree_tips(&repo, &*meta, &mut db)?;
        let mut ws = graph.into_workspace()?;
        let editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

        snapbox::assert_data_eq!(
            git_status_at_dir(repo.workdir().unwrap())?,
            str![[r#"
?? wt-detached/
?? wt/

"#]]
        );
        snapbox::assert_data_eq!(
            git_status_at_dir(&wt_dir)?,
            str![[r#"
 M a-file
?? new-file

"#]]
        );
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            str![[r#"
* 924b3a9 (feat) F1
| * 8f5cb92 (HEAD -> main) M1
|/  
| * 4bc8fd2 D1
|/  
* 35b8235 base

"#]]
        );
        snapbox::assert_data_eq!(
            tree_at(&repo, "feat")?,
            str![[r#"
e96e9b7
├── a-file:100644:4cb29ea "one\ntwo\nthree\n"
└── base:100644:df967b9 "base\n"

"#]]
            .raw()
        );

        let wt_repo = open_worktree_repo(&repo, "wt".into())?;
        let f1_id = repo.rev_parse_single("feat")?.detach();

        // Only the tracked modification - the untracked file must survive.
        let outcome = commit_amend(
            editor,
            f1_id,
            whole_file_spec("a-file"),
            0,
            ChangeSource::Worktree {
                repo: &wt_repo,
                name: "wt".into(),
            },
        )?;
        assert!(
            outcome.rejected_specs.is_empty(),
            "{:?}",
            outcome.rejected_specs
        );
        outcome.rebase.materialize(Default::default())?;

        snapbox::assert_data_eq!(
            git_status_at_dir(repo.workdir().unwrap())?,
            str![[r#"
?? wt-detached/
?? wt/

"#]]
        );
        // The merge-base override cancelled the consumed change during the worktree
        // checkout, while the file that wasn't amended stays dirty.
        snapbox::assert_data_eq!(
            git_status_at_dir(&wt_dir)?,
            str![[r#"
?? new-file

"#]]
        );
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            str![[r#"
* 22894d8 (feat) F1
| * 8f5cb92 (HEAD -> main) M1
|/  
| * 4bc8fd2 D1
|/  
* 35b8235 base

"#]]
        );
        snapbox::assert_data_eq!(
            tree_at(&repo, "feat")?,
            str![[r#"
45b12c2
├── a-file:100644:f384549 "one\ntwo\nthree\nfour\n"
└── base:100644:df967b9 "base\n"

"#]]
            .raw()
        );
        Ok(())
    }

    #[test]
    fn amend_one_worktree_hunk_leaves_the_other_hunk_dirty() -> Result<()> {
        let (repo, _tmp, mut meta, mut db) = scenario();
        let wt_dir = repo.workdir().expect("non-bare").join("wt");
        std::fs::write(wt_dir.join("a-file"), "ONE\ntwo\nthree\nfour\n")?;
        let graph = graph_with_worktree_tips(&repo, &*meta, &mut db)?;
        let mut ws = graph.into_workspace()?;
        let editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
        let wt_repo = open_worktree_repo(&repo, "wt".into())?;
        let change = but_core::diff::worktree_changes(&wt_repo)?
            .changes
            .into_iter()
            .find(|change| change.path == "a-file")
            .expect("a-file is modified");
        let but_core::UnifiedPatch::Patch { hunks, .. } = change
            .unified_patch(&wt_repo, 0)?
            .expect("text changes have a patch")
        else {
            panic!("text changes have a patch")
        };
        assert_eq!(hunks.len(), 2, "the fixture has two selectable hunks");
        let selected = DiffSpec {
            path: "a-file".into(),
            hunk_headers: vec![(&hunks[0]).into()],
            ..Default::default()
        };

        snapbox::assert_data_eq!(
            git_status_at_dir(&wt_dir)?,
            str![[r#"
 M a-file
?? new-file

"#]]
        );
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            str![[r#"
* 924b3a9 (feat) F1
| * 8f5cb92 (HEAD -> main) M1
|/  
| * 4bc8fd2 D1
|/  
* 35b8235 base

"#]]
        );

        let outcome = commit_amend(
            editor,
            repo.rev_parse_single("feat")?,
            vec![selected],
            0,
            ChangeSource::Worktree {
                repo: &wt_repo,
                name: "wt".into(),
            },
        )?;
        assert!(
            outcome.rejected_specs.is_empty(),
            "{:?}",
            outcome.rejected_specs
        );
        outcome.rebase.materialize(Default::default())?;

        // Only the selected hunk enters the commit.
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            str![[r#"
* 503639f (feat) F1
| * 8f5cb92 (HEAD -> main) M1
|/  
| * 4bc8fd2 D1
|/  
* 35b8235 base

"#]]
        );
        snapbox::assert_data_eq!(
            tree_at(&repo, "feat")?,
            str![[r#"
321f1dc
├── a-file:100644:59cb04e "ONE\ntwo\nthree\n"
└── base:100644:df967b9 "base\n"

"#]]
            .raw()
        );
        // The unselected hunk remains in the checkout.
        snapbox::assert_data_eq!(
            git_status_at_dir(&wt_dir)?,
            str![[r#"
 M a-file
?? new-file

"#]]
        );
        snapbox::assert_data_eq!(
            std::fs::read_to_string(wt_dir.join("a-file"))?,
            str![[r#"
ONE
two
three
four

"#]]
        );
        Ok(())
    }

    #[test]
    fn amend_into_another_branch_leaves_the_worktree_tip_alone() -> Result<()> {
        let (repo, _tmp, mut meta, mut db) = scenario();
        let wt_dir = repo.workdir().expect("non-bare").join("wt");
        let graph = graph_with_worktree_tips(&repo, &*meta, &mut db)?;
        let mut ws = graph.into_workspace()?;
        let editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

        snapbox::assert_data_eq!(
            git_status_at_dir(repo.workdir().unwrap())?,
            str![[r#"
?? wt-detached/
?? wt/

"#]]
        );
        snapbox::assert_data_eq!(
            git_status_at_dir(&wt_dir)?,
            str![[r#"
 M a-file
?? new-file

"#]]
        );
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            str![[r#"
* 924b3a9 (feat) F1
| * 8f5cb92 (HEAD -> main) M1
|/  
| * 4bc8fd2 D1
|/  
* 35b8235 base

"#]]
        );

        let wt_repo = open_worktree_repo(&repo, "wt".into())?;
        let m1_id = repo.head_id()?.detach();

        // The untracked addition applies cleanly onto a commit outside the
        // worktree's history.
        let outcome = commit_amend(
            editor,
            m1_id,
            whole_file_spec("new-file"),
            0,
            ChangeSource::Worktree {
                repo: &wt_repo,
                name: "wt".into(),
            },
        )?;
        assert!(
            outcome.rejected_specs.is_empty(),
            "{:?}",
            outcome.rejected_specs
        );
        outcome.rebase.materialize(Default::default())?;

        snapbox::assert_data_eq!(
            git_status_at_dir(repo.workdir().unwrap())?,
            str![[r#"
?? wt-detached/
?? wt/

"#]]
        );
        snapbox::assert_data_eq!(
            git_status_at_dir(&wt_dir)?,
            str![[r#"
 M a-file

"#]]
        );
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            str![[r#"
* 924b3a9 (feat) F1
| * 652a563 (HEAD -> main) M1
|/  
| * 4bc8fd2 D1
|/  
* 35b8235 base

"#]]
        );

        Ok(())
    }

    #[test]
    fn amend_into_an_immutable_commit_fails_fast() -> Result<()> {
        let (repo, _tmp, mut meta, mut db) = scenario();
        let graph = graph_with_worktree_tips(&repo, &*meta, &mut db)?;
        let mut ws = graph.into_workspace()?;
        let editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
        let wt_repo = open_worktree_repo(&repo, "wt".into())?;
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            str![[r#"
* 924b3a9 (feat) F1
| * 8f5cb92 (HEAD -> main) M1
|/  
| * 4bc8fd2 D1
|/  
* 35b8235 base

"#]]
        );

        // The detached worktree's commit is in the graph, but no branch points at
        // it, so it is never forced mutable. Amending into it used to write the
        // amended commit and report success while no ref ever adopted it.
        let err = commit_amend(
            editor,
            detached_tip(&repo)?,
            whole_file_spec("a-file"),
            0,
            ChangeSource::Worktree {
                repo: &wt_repo,
                name: "wt".into(),
            },
        )
        .unwrap_err();
        snapbox::assert_data_eq!(
            format!("{err:#}"),
            str![
                "cannot amend into 4bc8fd2eab2b190aa72cd5eca71b5f4c742d3621: the commit is immutable (not part of a mutable branch)"
            ]
        );

        // Nothing moved.
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            str![[r#"
* 924b3a9 (feat) F1
| * 8f5cb92 (HEAD -> main) M1
|/  
| * 4bc8fd2 D1
|/  
* 35b8235 base

"#]]
        );
        Ok(())
    }

    #[test]
    fn amend_from_an_unknown_worktree_fails_without_moving_refs() -> Result<()> {
        let (repo, _tmp, mut meta, mut db) = scenario();
        let graph = graph_with_worktree_tips(&repo, &*meta, &mut db)?;
        let mut ws = graph.into_workspace()?;
        let editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
        let wt_repo = open_worktree_repo(&repo, "wt".into())?;
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            str![[r#"
* 924b3a9 (feat) F1
| * 8f5cb92 (HEAD -> main) M1
|/  
| * 4bc8fd2 D1
|/  
* 35b8235 base

"#]]
        );

        let err = commit_amend(
            editor,
            repo.rev_parse_single("feat")?.detach(),
            whole_file_spec("a-file"),
            0,
            ChangeSource::Worktree {
                repo: &wt_repo,
                name: "not-a-worktree".into(),
            },
        )
        .unwrap_err();
        snapbox::assert_data_eq!(
            format!("{err:#}"),
            str!["Worktree not-a-worktree has no checkout recorded in the editor"]
        );

        // Nothing moved.
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            str![[r#"
* 924b3a9 (feat) F1
| * 8f5cb92 (HEAD -> main) M1
|/  
| * 4bc8fd2 D1
|/  
* 35b8235 base

"#]]
        );
        Ok(())
    }

    #[test]
    fn commit_create_from_a_worktree_moves_its_branch_and_checkout() -> Result<()> {
        let (repo, _tmp, mut meta, mut db) = scenario();
        let wt_dir = repo.workdir().expect("non-bare").join("wt");
        let graph = graph_with_worktree_tips(&repo, &*meta, &mut db)?;
        let mut ws = graph.into_workspace()?;
        let editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
        let wt_repo = open_worktree_repo(&repo, "wt".into())?;

        snapbox::assert_data_eq!(
            git_status_at_dir(&wt_dir)?,
            str![[r#"
 M a-file
?? new-file

"#]]
        );
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            str![[r#"
* 924b3a9 (feat) F1
| * 8f5cb92 (HEAD -> main) M1
|/  
| * 4bc8fd2 D1
|/  
* 35b8235 base

"#]]
        );

        let outcome = commit_create(
            editor,
            whole_file_spec("a-file"),
            but_rebase::graph_rebase::mutate::RelativeTo::Reference("refs/heads/feat".try_into()?),
            InsertSide::Below,
            "F2",
            0,
            ChangeSource::Worktree {
                repo: &wt_repo,
                name: "wt".into(),
            },
        )?;
        assert!(
            outcome.rejected_specs.is_empty(),
            "{:?}",
            outcome.rejected_specs
        );
        outcome.rebase.materialize(Default::default())?;

        // The new commit sits on top of the worktree's previous tip, and its branch moved there.
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            str![[r#"
* 6599517 (feat) F2
* 924b3a9 F1
| * 8f5cb92 (HEAD -> main) M1
|/  
| * 4bc8fd2 D1
|/  
* 35b8235 base

"#]]
        );
        snapbox::assert_data_eq!(
            tree_at(&repo, "feat")?,
            str![[r#"
45b12c2
├── a-file:100644:f384549 "one\ntwo\nthree\nfour\n"
└── base:100644:df967b9 "base\n"

"#]]
            .raw()
        );
        // The committed change was cancelled from the worktree, the untracked file survives.
        snapbox::assert_data_eq!(
            git_status_at_dir(&wt_dir)?,
            str![[r#"
?? new-file

"#]]
        );
        Ok(())
    }
}
