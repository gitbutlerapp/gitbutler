//! Snapshot tests for the [`GraphWorkspace`] projection, covering the
//! permutations of [`Editor::graph_workspace`]:
//!
//! - on the workspace branch with a workspace commit, split into one or more
//!   stacks (with and without a target bounding them);
//! - stacks that share ancestry and therefore collapse into a single stack;
//! - pegged onto a non-workspace branch (one stack of everything), with and
//!   without a target;
//! - on the workspace branch but with no discoverable workspace commit.
//!
//! [`GraphWorkspace`]: but_rebase::graph_rebase::GraphWorkspace

use anyhow::Result;
use but_core::ref_metadata::ProjectMeta;
use but_graph::Graph;
use but_rebase::graph_rebase::Editor;
use but_testsupport::visualize_commit_graph_all;

use crate::utils::{fixture_writable, standard_options};

/// Build an editor for `fixture` (optionally bounded by `target`, a revspec
/// resolved against the repo) and render its [`Editor::graph_workspace`]
/// projection. All borrows stay local so callers just snapshot the string.
fn render(fixture: &str, target: Option<&str>) -> Result<String> {
    let (repo, _tmp, mut meta) = fixture_writable(fixture)?;

    let graph =
        Graph::from_head(&repo, &*meta, ProjectMeta::default(), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;

    // The projection bounds stacks at the target commit, so wire it onto the
    // workspace graph that the editor reads from.
    ws.graph.project_meta = ProjectMeta {
        target_commit_id: target
            .map(|t| repo.rev_parse_single(t).map(|id| id.detach()))
            .transpose()?,
        ..Default::default()
    };
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    editor.graph_workspace_ascii()
}

/// A linear workspace (base→a→b→c under the workspace commit) with no target,
/// so the single stack reaches all the way down to `base`.
#[test]
fn single_stack_no_target() -> Result<()> {
    let (repo, _tmp, _meta) = fixture_writable("workspace-signed")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 8795f47 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * dd72792 (main, c) c
    * e5aa7b5 (b) b
    * 3bfeb52 (a) a
    * b6e2f57 (base) base
    ");

    insta::assert_snapshot!(render("workspace-signed", None)?, @"
    # Above workspace
    ◎  refs/heads/gitbutler/workspace

    # Workspace commit
    ●  8795f47 GitButler Workspace Commit

    # Stack 0
    ◎  refs/heads/c
    ◎  refs/heads/main
    ●  dd72792 c
    ◎  refs/heads/b
    ●  e5aa7b5 b
    ◎  refs/heads/a
    ●  3bfeb52 a
    ◎  refs/heads/base
    ●  b6e2f57 base
    ");
    Ok(())
}

/// The same linear workspace bounded by a target at `base`: the stack now stops
/// above `base`, and `base` is no longer part of the projection.
#[test]
fn single_stack_with_target() -> Result<()> {
    insta::assert_snapshot!(render("workspace-signed", Some("base"))?, @"
    # Above workspace
    ◎  refs/heads/gitbutler/workspace

    # Workspace commit
    ●  8795f47 GitButler Workspace Commit

    # Stack 0
    ◎  refs/heads/c
    ◎  refs/heads/main
    ●  dd72792 c
    ◎  refs/heads/b
    ●  e5aa7b5 b
    ◎  refs/heads/a
    ●  3bfeb52 a
    ◎  refs/heads/base
    ");
    Ok(())
}

/// Two parents of the workspace commit whose histories overlap (stack-2's tip
/// is an ancestor of stack-1's tip). With no target they share ancestry, so the
/// de-duplication merges them into a single stack.
#[test]
fn overlapping_stacks_merge_into_one() -> Result<()> {
    let (repo, _tmp, _meta) = fixture_writable("workspace-with-empty-stack")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   74bcc92 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    * | 2169646 (stack-1) Commit D
    * | 46ef828 Commit C
    |/  
    | * a0f2ac5 (origin/main, main) Commit X
    |/  
    * f555940 (stack-2) Commit A
    * d664be0 Commit B
    * fafd9d0 init
    ");

    insta::assert_snapshot!(render("workspace-with-empty-stack", None)?, @"
    # Above workspace
    ◎  refs/heads/gitbutler/workspace

    # Workspace commit
    ●  74bcc92 GitButler Workspace Commit

    # Stack 0
    ◎  refs/heads/stack-1
    ●  2169646 Commit D
    ●  46ef828 Commit C
    ◎  refs/heads/stack-2
    ●  f555940 Commit A
    ●  d664be0 Commit B
    ●  fafd9d0 init
    ");
    Ok(())
}

/// Three stacks that all point at the same base commit. They share that node,
/// so they collapse into a single stack.
#[test]
fn three_stacks_same_base_collapse() -> Result<()> {
    let (repo, _tmp, _meta) = fixture_writable("workspace-with-three-empty-stacks")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * a26ae77 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * 1cf9cf4 (origin/main, main) Commit X
    |/  
    * fafd9d0 (stack-3, stack-2, stack-1) init
    ");

    insta::assert_snapshot!(render("workspace-with-three-empty-stacks", None)?, @"
    # Above workspace
    ◎  refs/heads/gitbutler/workspace

    # Workspace commit
    ●  a26ae77 GitButler Workspace Commit

    # Stack 0
    ◎  refs/heads/stack-1
    ◎  refs/heads/stack-2
    ◎  refs/heads/stack-3
    ●  fafd9d0 init
    ");
    Ok(())
}

/// Two divergent branches sharing `base`, bounded by a target at `base`.
///
/// They still merge into a single stack: the target excludes the base *commit*,
/// but the `main`/`origin/main` *ref node* sitting just above it survives and is
/// reachable from both branches, so they share a node and collapse. A target
/// alone does not separate stacks that branch off a common ref.
#[test]
fn divergent_stacks_sharing_base_merge_with_target() -> Result<()> {
    let (repo, _tmp, _meta) = fixture_writable("workspace-two-stacks")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   1162583 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * afc3f8f (stack-b) B2
    | * b3ee99c B1
    * | 49c06ff (stack-a) A2
    * | ff76d2f A1
    |/  
    * 965998b (origin/main, main) base
    ");

    insta::assert_snapshot!(render("workspace-two-stacks", Some("main"))?, @"
    # Above workspace
    ◎  refs/heads/gitbutler/workspace

    # Workspace commit
    ●  1162583 GitButler Workspace Commit

    # Stack 0
    ◎  refs/heads/stack-a
    ●  49c06ff A2
    ●  ff76d2f A1
    │ ◎  refs/heads/stack-b
    │ ●  afc3f8f B2
    │ ●  b3ee99c B1
    ├─╯
    ◎  refs/heads/main
    ");
    Ok(())
}

/// The same divergent fixture with no target: both stacks reach `base`, share
/// it (and the refs above it), and therefore merge into a single stack.
#[test]
fn divergent_stacks_sharing_base_merge() -> Result<()> {
    insta::assert_snapshot!(render("workspace-two-stacks", None)?, @"
    # Above workspace
    ◎  refs/heads/gitbutler/workspace

    # Workspace commit
    ●  1162583 GitButler Workspace Commit

    # Stack 0
    ◎  refs/heads/stack-a
    ●  49c06ff A2
    ●  ff76d2f A1
    │ ◎  refs/heads/stack-b
    │ ●  afc3f8f B2
    │ ●  b3ee99c B1
    ├─╯
    ◎  refs/heads/main
    ●  965998b base
    ");
    Ok(())
}

/// Pegged onto `main` (not the workspace branch): there is no workspace commit
/// and nothing above the workspace; everything from HEAD lands in one stack.
#[test]
fn pegged_no_target() -> Result<()> {
    let (repo, _tmp, _meta) = fixture_writable("four-commits")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 120e3a9 (HEAD -> main) c
    * a96434e b
    * d591dfe a
    * 35b8235 base
    ");

    insta::assert_snapshot!(render("four-commits", None)?, @"
    # Above workspace
    (empty)

    # Workspace commit
    (empty)

    # Stack 0
    ◎  refs/heads/main
    ●  120e3a9 c
    ●  a96434e b
    ●  d591dfe a
    ●  35b8235 base
    ");
    Ok(())
}

/// Pegged onto `main` with a target at `base`: the single stack is bounded
/// above `base`.
#[test]
fn pegged_with_target() -> Result<()> {
    insta::assert_snapshot!(render("four-commits", Some("main~3"))?, @"
    # Above workspace
    (empty)

    # Workspace commit
    (empty)

    # Stack 0
    ◎  refs/heads/main
    ●  120e3a9 c
    ●  a96434e b
    ●  d591dfe a
    ");
    Ok(())
}

/// Two stacks with no shared history at all (`stack-b` is an orphan root). They
/// share no node, so - unlike the base-sharing case above - they stay genuinely
/// separate, even without a target.
#[test]
fn disjoint_stacks_stay_separate() -> Result<()> {
    let (repo, _tmp, _meta) = fixture_writable("workspace-disjoint-stacks")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   f97c026 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * cb7021b (stack-b) B2
    | * ce3278a B1
    * 49c06ff (stack-a) A2
    * ff76d2f A1
    * 965998b (origin/main, main) base
    ");

    insta::assert_snapshot!(render("workspace-disjoint-stacks", None)?, @"
    # Above workspace
    ◎  refs/heads/gitbutler/workspace

    # Workspace commit
    ●  f97c026 GitButler Workspace Commit

    # Stack 0
    ◎  refs/heads/stack-b
    ●  cb7021b B2
    ●  ce3278a B1

    # Stack 1
    ◎  refs/heads/stack-a
    ●  49c06ff A2
    ●  ff76d2f A1
    ◎  refs/heads/main
    ●  965998b base
    ");
    Ok(())
}

/// The direct contrast to `divergent_stacks_sharing_base_merge_with_target`:
/// the *same* target (`main`), but the two stacks share no node, so they stay
/// separate. This is the shape that sidesteps the known limitation documented on
/// `GraphWorkspace::stacks` - the target trims `base` off `stack-a` without the
/// shared `main` ref node collapsing the two stacks together.
#[test]
fn disjoint_stacks_stay_separate_with_target() -> Result<()> {
    insta::assert_snapshot!(render("workspace-disjoint-stacks", Some("main"))?, @"
    # Above workspace
    ◎  refs/heads/gitbutler/workspace

    # Workspace commit
    ●  f97c026 GitButler Workspace Commit

    # Stack 0
    ◎  refs/heads/stack-b
    ●  cb7021b B2
    ●  ce3278a B1

    # Stack 1
    ◎  refs/heads/stack-a
    ●  49c06ff A2
    ●  ff76d2f A1
    ◎  refs/heads/main
    ");
    Ok(())
}

/// On the workspace branch but the tip is a plain commit, not a managed
/// workspace commit: no workspace commit is found, so everything reachable from
/// HEAD lands in `above_workspace` and there are no stacks.
#[test]
fn workspace_branch_without_managed_commit() -> Result<()> {
    let (repo, _tmp, _meta) = fixture_writable("workspace-without-managed-commit")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 1b78c63 (HEAD -> gitbutler/workspace) just a normal commit
    * 4d41a5c (origin/main, main) one
    * 965998b base
    ");

    insta::assert_snapshot!(render("workspace-without-managed-commit", None)?, @"
    # Above workspace
    ◎  refs/heads/gitbutler/workspace
    ●  1b78c63 just a normal commit
    ◎  refs/heads/main
    ●  4d41a5c one
    ●  965998b base

    # Workspace commit
    (empty)
    ");
    Ok(())
}
