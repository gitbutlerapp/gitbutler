use std::vec;

use anyhow::{Result, bail};
use bstr::ByteSlice;
use but_core::{ChangeId, Commit, commit::Headers};
use but_testsupport::gix_testtools::tempfile::TempDir;
use but_testsupport::{InMemoryRefMetadata, visualize_commit_graph_all, visualize_tree};
use but_workspace::branch::integrate_branch_upstream::{
    BranchIntegrationStrategy, InitialBranchIntegration, IntegrationDivergenceCommit,
    IntegrationDivergenceTargetRelation, InteractiveIntegration, InteractiveIntegrationStep,
    get_initial_integration_steps_for_branch, integrate_branch_with_steps,
};
use but_workspace::resolve_tracking_branch_ref_name;
use gix::prelude::ObjectIdExt;

use crate::{
    ref_info::with_workspace_commit::utils::{
        StackState, add_stack_with_segments, named_writable_scenario_with_description_and_graph,
    },
    utils::{read_only_in_memory_scenario, read_only_in_memory_scenario_named},
};

fn normalized_graph_snapshot(repo: &gix::Repository) -> Result<String> {
    let rendered = visualize_commit_graph_all(repo)?;
    Ok(rendered
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n"))
}

fn normalized_tree_snapshot(tree_id: gix::ObjectId, repo: &gix::Repository) -> Result<String> {
    Ok(visualize_tree(tree_id.attach(repo))
        .to_string()
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n"))
}

fn labeled_integration_snapshot(
    integration: &InteractiveIntegration,
    labels: &[(gix::ObjectId, &str)],
) -> String {
    let mut out = String::new();
    out.push_str("merge-base ");
    out.push_str(&label_for(integration.merge_base, labels));
    out.push('\n');

    for step in &integration.steps {
        match step {
            InteractiveIntegrationStep::Pick { commit_id } => {
                out.push_str("pick ");
                out.push_str(&label_for(*commit_id, labels));
            }
            InteractiveIntegrationStep::Merge { commit_id } => {
                out.push_str("merge ");
                out.push_str(&label_for(*commit_id, labels));
            }
            InteractiveIntegrationStep::Squash { commits, message } => {
                out.push_str("squash");
                for commit_id in commits {
                    out.push(' ');
                    out.push_str(&label_for(*commit_id, labels));
                }
                if let Some(message) = message {
                    out.push_str(" | message=");
                    out.push_str(&format!("{message:?}"));
                }
            }
        }
        out.push('\n');
    }

    out.lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

fn labeled_divergence_snapshot(
    initial: &InitialBranchIntegration,
    labels: &[(gix::ObjectId, &str)],
) -> String {
    fn render_commit(
        prefix: &str,
        commit: &IntegrationDivergenceCommit,
        labels: &[(gix::ObjectId, &str)],
    ) -> String {
        let refs = if commit.refs.is_empty() {
            String::new()
        } else {
            format!(" ({})", commit.refs.join(", "))
        };
        let relation = match &commit.target_relation {
            IntegrationDivergenceTargetRelation::NotIntegrated => String::new(),
            IntegrationDivergenceTargetRelation::HistoricallyIntegrated { target_commit_id } => {
                format!(
                    " [historically-integrated:{}]",
                    label_for(*target_commit_id, labels)
                )
            }
        };
        format!(
            "{prefix}{}{}{} {}",
            label_for(commit.id, labels),
            refs,
            relation,
            commit.subject
        )
    }

    let mut out = Vec::new();
    for commit in &initial.divergence.local_only {
        out.push(render_commit("* ", commit, labels));
    }
    for commit in &initial.divergence.upstream_only {
        let prefix = if initial.divergence.local_only.is_empty() {
            "* "
        } else {
            "| * "
        };
        out.push(render_commit(prefix, commit, labels));
    }
    if !initial.divergence.local_only.is_empty() && !initial.divergence.upstream_only.is_empty() {
        out.push("|/".into());
    }
    out.push(render_commit("* ", &initial.divergence.merge_base, labels));
    out.join("\n")
}

fn labeled_graph_snapshot(
    repo: &gix::Repository,
    labels: &[(gix::ObjectId, &str)],
) -> Result<String> {
    let mut snapshot = normalized_graph_snapshot(repo)?;
    for (id, label) in labels {
        let short_id = id.to_string();
        let short_id = &short_id[..7];
        snapshot = snapshot.replace(short_id, label);
    }
    Ok(snapshot)
}

fn label_for(id: gix::ObjectId, labels: &[(gix::ObjectId, &str)]) -> String {
    labels
        .iter()
        .find_map(|(candidate, label)| (*candidate == id).then_some(*label))
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| id.to_string())
}

fn initial_integration_for_branch(
    ref_name: &gix::refs::FullNameRef,
    repo: &gix::Repository,
    target_ref_name: Option<&gix::refs::FullNameRef>,
) -> Result<InitialBranchIntegration> {
    initial_integration_for_branch_with_strategy(
        ref_name,
        repo,
        target_ref_name,
        BranchIntegrationStrategy::PullRebase,
    )
}

fn initial_integration_for_branch_with_strategy(
    ref_name: &gix::refs::FullNameRef,
    repo: &gix::Repository,
    target_ref_name: Option<&gix::refs::FullNameRef>,
    strategy: BranchIntegrationStrategy,
) -> Result<InitialBranchIntegration> {
    let mut meta = InMemoryRefMetadata::default();
    let graph = integration_graph_for_branch(ref_name, repo, target_ref_name, &meta)?;
    let mut workspace = graph.into_workspace()?;
    get_initial_integration_steps_for_branch(ref_name, strategy, &mut workspace, &mut meta, repo)
}

fn integration_graph_for_branch(
    ref_name: &gix::refs::FullNameRef,
    repo: &gix::Repository,
    target_ref_name: Option<&gix::refs::FullNameRef>,
    meta: &InMemoryRefMetadata,
) -> Result<but_graph::Graph> {
    if let Some(target_ref_name) = target_ref_name {
        let head = repo.head()?;
        let (head_id, head_ref_name) = match head.kind {
            gix::head::Kind::Symbolic(reference) => {
                let ref_name = reference.name;
                let head_id = repo.find_reference(ref_name.as_ref())?.id().detach();
                (head_id, Some(ref_name))
            }
            gix::head::Kind::Detached { target, peeled } => (peeled.unwrap_or(target), None),
            gix::head::Kind::Unborn(_) => bail!("test repositories should not be unborn"),
        };
        let target_id = repo.find_reference(target_ref_name)?.id().detach();
        let upstream_ref_name = resolve_tracking_branch_ref_name(ref_name, repo)?;
        let upstream_id = repo
            .find_reference(upstream_ref_name.as_ref())?
            .id()
            .detach();
        but_graph::Graph::from_commit_traversal_tips(
            repo,
            [
                but_graph::init::Tip::entrypoint(head_id, head_ref_name),
                but_graph::init::Tip::reachable(upstream_id, Some(upstream_ref_name.into_owned())),
                but_graph::init::Tip::integrated(target_id, Some(target_ref_name.to_owned())),
            ],
            meta,
            Default::default(),
        )
    } else if let Ok(upstream_ref_name) = resolve_tracking_branch_ref_name(ref_name, repo) {
        let head = repo.head()?;
        let (head_id, head_ref_name) = match head.kind {
            gix::head::Kind::Symbolic(reference) => {
                let ref_name = reference.name;
                let head_id = repo.find_reference(ref_name.as_ref())?.id().detach();
                (head_id, Some(ref_name))
            }
            gix::head::Kind::Detached { target, peeled } => (peeled.unwrap_or(target), None),
            gix::head::Kind::Unborn(_) => bail!("test repositories should not be unborn"),
        };
        let upstream_id = repo
            .find_reference(upstream_ref_name.as_ref())?
            .id()
            .detach();
        but_graph::Graph::from_commit_traversal_tips(
            repo,
            [
                but_graph::init::Tip::entrypoint(head_id, head_ref_name),
                but_graph::init::Tip::reachable(upstream_id, Some(upstream_ref_name.into_owned())),
            ],
            meta,
            Default::default(),
        )
    } else {
        but_graph::Graph::from_head(repo, meta, Default::default())
    }
}

fn integration_workspace_for_branch(
    ref_name: &gix::refs::FullNameRef,
    repo: &gix::Repository,
    target_ref_name: Option<&gix::refs::FullNameRef>,
) -> Result<(but_graph::Workspace, InMemoryRefMetadata)> {
    let meta = InMemoryRefMetadata::default();
    let graph = integration_graph_for_branch(ref_name, repo, target_ref_name, &meta)?;
    Ok((graph.into_workspace()?, meta))
}

#[test]
fn errors_when_branch_has_no_tracking_branch() -> Result<()> {
    let repo = read_only_in_memory_scenario("merge-with-two-branches-line-offset")
        .expect("fixture repo should be available");

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @r"
    *   2a6d103 (HEAD -> merge) Merge branch 'A' into merge
    |\
    | * 7f389ed (A) add 10 to the beginning
    * | 91ef6f6 (B) add 10 to the end
    |/
    * ff045ef (main) init
    ");

    let err = initial_integration_for_branch(r("refs/heads/A"), &repo, None)
        .expect_err("branch without tracking must fail");

    assert!(
        err.to_string().contains("has no tracking branch"),
        "unexpected error: {err:#}"
    );

    Ok(())
}

#[test]
fn partitions_diverged_branch_into_application_order() -> Result<()> {
    let mut repo =
        read_only_in_memory_scenario_named("with-remotes-no-workspace", "remote-diverged")?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * 1a265a4 (HEAD -> A) local change in A
    | * 89cc2d3 (origin/A) change in A
    |/
    * d79bba9 new file in A
    * c166d42 (origin/main, origin/HEAD, main) init-integration
    ");

    let local_tip = repo.rev_parse_single("A")?.detach();
    let upstream_tip = repo.rev_parse_single("origin/A")?.detach();
    let merge_base = repo.rev_parse_single("A~1")?.detach();
    configure_tracking_for_branch_a(&mut repo)?;

    let initial = initial_integration_for_branch(
        r("refs/heads/A"),
        &repo,
        Some(r("refs/remotes/origin/main")),
    )?;

    insta::assert_snapshot!(
        labeled_integration_snapshot(
            &initial.integration,
            &[
                (merge_base, "merge-base"),
                (local_tip, "local-tip"),
                (upstream_tip, "upstream-tip"),
            ]
        ),
        @"
    merge-base merge-base
    pick upstream-tip
    pick local-tip
    "
    );

    insta::assert_snapshot!(
        labeled_divergence_snapshot(
            &initial,
            &[
                (merge_base, "merge-base"),
                (local_tip, "local-tip"),
                (upstream_tip, "upstream-tip"),
            ]
        ),
        @"
        * local-tip (A) local change in A
        | * upstream-tip (origin/A) change in A
        |/
        * merge-base new file in A
        "
    );

    let step_ids = pick_step_ids(&initial.integration.steps);

    assert_eq!(
        step_ids,
        vec![upstream_tip, local_tip],
        "expected application order to replay the upstream commit before the local tip"
    );
    Ok(())
}

#[test]
fn falls_back_to_unique_remote_branch_without_tracking_config() -> Result<()> {
    let repo = read_only_in_memory_scenario_named("with-remotes-no-workspace", "remote-diverged")?;

    let local_tip = repo.rev_parse_single("A")?.detach();
    let upstream_tip = repo.rev_parse_single("origin/A")?.detach();
    let merge_base = repo.rev_parse_single("A~1")?.detach();

    let initial = initial_integration_for_branch(r("refs/heads/A"), &repo, None)?;

    insta::assert_snapshot!(
        labeled_integration_snapshot(
            &initial.integration,
            &[
                (merge_base, "merge-base"),
                (local_tip, "local-tip"),
                (upstream_tip, "upstream-tip"),
            ]
        ),
        @"
    merge-base merge-base
    pick upstream-tip
    pick local-tip
    "
    );

    insta::assert_snapshot!(
        labeled_divergence_snapshot(
            &initial,
            &[
                (merge_base, "merge-base"),
                (local_tip, "local-tip"),
                (upstream_tip, "upstream-tip"),
            ]
        ),
        @"
        * local-tip (A) local change in A
        | * upstream-tip (origin/A) change in A
        |/
        * merge-base new file in A
        "
    );

    Ok(())
}

#[test]
fn keeps_graph_truth_even_when_change_ids_match() -> Result<()> {
    let mut repo = read_only_in_memory_scenario_named(
        "journey03",
        "01-rewritten-local-commit-is-paired-with-remote",
    )?;
    configure_tracking_for_branch_a(&mut repo)?;
    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * 0b1ed50 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * e9c9d74 (A) A2
    * 550b6ac A1
    | * ad92cce (origin/A) A2
    | * e1f216e A1
    |/
    * fafd9d0 (origin/main, main) init
    ");

    let local_only = repo.rev_parse_single("A~1")?.detach();
    let remote_only = repo.rev_parse_single("origin/A~1")?.detach();
    let local_tip = repo.rev_parse_single("A")?.detach();
    let remote_tip = repo.rev_parse_single("origin/A")?.detach();
    let merge_base = repo.rev_parse_single("A~2")?.detach();
    configure_tracking_for_branch_a(&mut repo)?;

    let initial = initial_integration_for_branch(
        r("refs/heads/A"),
        &repo,
        Some(r("refs/remotes/origin/main")),
    )?;

    insta::assert_snapshot!(
        labeled_integration_snapshot(
            &initial.integration,
            &[
                (merge_base, "merge-base"),
                (local_only, "local-only"),
                (remote_only, "remote-only"),
                (local_tip, "local-tip"),
                (remote_tip, "remote-tip"),
            ]
        ),
        @"
    merge-base merge-base
    pick remote-only
    pick remote-tip
    pick local-only
    pick local-tip
    "
    );

    insta::assert_snapshot!(
        labeled_divergence_snapshot(
            &initial,
            &[
                (merge_base, "merge-base"),
                (local_only, "local-only"),
                (remote_only, "remote-only"),
                (local_tip, "local-tip"),
                (remote_tip, "remote-tip"),
            ]
        ),
        @"
    * local-tip (A) A2
    * local-only A1
    | * remote-tip (origin/A) A2
    | * remote-only A1
    |/
    * merge-base [historically-integrated:merge-base] init
    "
    );

    let step_ids = pick_step_ids(&initial.integration.steps);

    assert_eq!(
        step_ids,
        vec![remote_only, remote_tip, local_only, local_tip],
        "expected application order to follow the graph, even for rewritten commits"
    );
    Ok(())
}

#[test]
fn initial_steps_strategy_variants_are_distinct_presets() -> Result<()> {
    let mut repo = read_only_in_memory_scenario_named(
        "journey03",
        "01-rewritten-local-commit-is-paired-with-remote",
    )?;
    configure_tracking_for_branch_a(&mut repo)?;

    let local_only = repo.rev_parse_single("A~1")?.detach();
    let remote_only = repo.rev_parse_single("origin/A~1")?.detach();
    let local_tip = repo.rev_parse_single("A")?.detach();
    let remote_tip = repo.rev_parse_single("origin/A")?.detach();
    let merge_base = repo.rev_parse_single("A~2")?.detach();
    let labels = [
        (merge_base, "merge-base"),
        (local_only, "local-only"),
        (remote_only, "remote-only"),
        (local_tip, "local-tip"),
        (remote_tip, "remote-tip"),
    ];

    let mut snapshot = String::new();
    for (name, strategy) in [
        ("pull-rebase", BranchIntegrationStrategy::PullRebase),
        ("merge", BranchIntegrationStrategy::Merge),
        ("pick-remote", BranchIntegrationStrategy::PickRemote),
    ] {
        let initial = initial_integration_for_branch_with_strategy(
            r("refs/heads/A"),
            &repo,
            Some(r("refs/remotes/origin/main")),
            strategy,
        )?;
        snapshot.push_str(name);
        snapshot.push('\n');
        snapshot.push_str(&labeled_integration_snapshot(&initial.integration, &labels));
        snapshot.push_str("\n\n");
    }

    insta::assert_snapshot!(
        snapshot.trim_end(),
        "strategy presets should remain distinct and ordered for editable integration",
        @"
    pull-rebase
    merge-base merge-base
    pick remote-only
    pick remote-tip
    pick local-only
    pick local-tip

    merge
    merge-base merge-base
    pick local-only
    pick local-tip
    merge remote-tip

    pick-remote
    merge-base merge-base
    pick remote-only
    pick remote-tip
    "
    );

    Ok(())
}

#[test]
fn initial_steps_smart_squash_folds_matching_upstream_commits_into_child_most_local_commit()
-> Result<()> {
    let (_tmp, mut repo) = build_smart_squash_example_repo()?;
    configure_tracking_for_branch_a(&mut repo)?;
    let change_id = ChangeId::from_number_for_testing(1);
    let change_id_string = change_id.to_string();

    let [local_parent, local_tip] = rewrite_commits_with_change_ids(
        &repo,
        "refs/heads/A",
        &["A~1", "A"],
        &[Some(change_id.clone()), Some(change_id.clone())],
    )?
    .try_into()
    .expect("rewrote two local commits");
    let [remote_parent, remote_tip] = rewrite_commits_with_change_ids(
        &repo,
        "refs/remotes/origin/A",
        &["origin/A~1", "origin/A"],
        &[Some(change_id.clone()), Some(change_id)],
    )?
    .try_into()
    .expect("rewrote two upstream commits");
    let merge_base = repo.rev_parse_single("A~2")?.detach();

    let initial = initial_integration_for_branch_with_strategy(
        r("refs/heads/A"),
        &repo,
        Some(r("refs/remotes/origin/main")),
        BranchIntegrationStrategy::SmartSquash,
    )?;

    insta::assert_snapshot!(
        labeled_integration_snapshot(
            &initial.integration,
            &[
                (merge_base, "merge-base"),
                (local_parent, "local-parent"),
                (local_tip, "local-tip"),
                (remote_parent, "remote-parent"),
                (remote_tip, "remote-tip"),
            ]
        ),
        "smart-squash should fold matched upstream commits into the child-most matching local commit",
        @"
    merge-base merge-base
    pick local-parent
    squash local-tip remote-parent remote-tip
    "
    );

    assert_eq!(
        initial
            .integration
            .steps
            .iter()
            .filter(|step| matches!(step, InteractiveIntegrationStep::Pick { commit_id } if *commit_id == remote_parent || *commit_id == remote_tip))
            .count(),
        0,
        "matched upstream commits should not also be emitted as standalone picks"
    );
    let local_tip_display = initial
        .divergence
        .local_only
        .iter()
        .find(|commit| commit.id == local_tip)
        .expect("local tip should be visible in divergence rows");
    assert_eq!(
        local_tip_display.change_id.as_deref(),
        Some(change_id_string.as_str()),
        "divergence rows should expose explicit Change-Ids"
    );
    assert!(
        local_tip_display.created_at > 0,
        "divergence rows should expose commit timestamps"
    );
    assert!(
        !local_tip_display.author.name.is_empty(),
        "divergence rows should expose commit authors"
    );

    Ok(())
}

#[test]
fn initial_steps_smart_squash_falls_back_to_pull_rebase_without_matching_explicit_change_ids()
-> Result<()> {
    let (_tmp, mut repo) = build_smart_squash_example_repo()?;
    configure_tracking_for_branch_a(&mut repo)?;

    let local_parent = repo.rev_parse_single("A~1")?.detach();
    let remote_parent = repo.rev_parse_single("origin/A~1")?.detach();
    let local_tip = repo.rev_parse_single("A")?.detach();
    let remote_tip = repo.rev_parse_single("origin/A")?.detach();
    let merge_base = repo.rev_parse_single("A~2")?.detach();
    let labels = [
        (merge_base, "merge-base"),
        (local_parent, "local-parent"),
        (local_tip, "local-tip"),
        (remote_parent, "remote-parent"),
        (remote_tip, "remote-tip"),
    ];

    let pull_rebase = initial_integration_for_branch_with_strategy(
        r("refs/heads/A"),
        &repo,
        Some(r("refs/remotes/origin/main")),
        BranchIntegrationStrategy::PullRebase,
    )?;
    let smart_squash = initial_integration_for_branch_with_strategy(
        r("refs/heads/A"),
        &repo,
        Some(r("refs/remotes/origin/main")),
        BranchIntegrationStrategy::SmartSquash,
    )?;

    insta::assert_snapshot!(
        labeled_integration_snapshot(&smart_squash.integration, &labels),
        "smart-squash should match pull-rebase when there are no explicit Change-Id headers",
        @"
    merge-base merge-base
    pick remote-parent
    pick remote-tip
    pick local-parent
    pick local-tip
    "
    );
    assert_eq!(
        labeled_integration_snapshot(&smart_squash.integration, &labels),
        labeled_integration_snapshot(&pull_rebase.integration, &labels),
        "headerless commits should not use synthetic Change-Ids for smart-squash matching",
    );

    let [local_parent, local_tip] = rewrite_commits_with_change_ids(
        &repo,
        "refs/heads/A",
        &["A~1", "A"],
        &[
            Some(ChangeId::from_number_for_testing(1)),
            Some(ChangeId::from_number_for_testing(2)),
        ],
    )?
    .try_into()
    .expect("rewrote two local commits");
    let [remote_parent, remote_tip] = rewrite_commits_with_change_ids(
        &repo,
        "refs/remotes/origin/A",
        &["origin/A~1", "origin/A"],
        &[
            Some(ChangeId::from_number_for_testing(3)),
            Some(ChangeId::from_number_for_testing(4)),
        ],
    )?
    .try_into()
    .expect("rewrote two upstream commits");
    let labels = [
        (merge_base, "merge-base"),
        (local_parent, "local-parent"),
        (local_tip, "local-tip"),
        (remote_parent, "remote-parent"),
        (remote_tip, "remote-tip"),
    ];
    let smart_squash = initial_integration_for_branch_with_strategy(
        r("refs/heads/A"),
        &repo,
        Some(r("refs/remotes/origin/main")),
        BranchIntegrationStrategy::SmartSquash,
    )?;

    insta::assert_snapshot!(
        labeled_integration_snapshot(&smart_squash.integration, &labels),
        "smart-squash should match pull-rebase when explicit Change-Ids do not match",
        @"
    merge-base merge-base
    pick remote-parent
    pick remote-tip
    pick local-parent
    pick local-tip
    "
    );

    Ok(())
}

#[test]
fn initial_steps_smart_squash_does_not_target_integrated_local_commits() -> Result<()> {
    let (_tmp, mut repo) = build_branch_integration_example_repo(
        ExampleScenario::LocalCommitHistoricallyIntegratedOnTarget,
    )?;
    configure_tracking_for_branch_a(&mut repo)?;

    let integrated_change_id = ChangeId::from_number_for_testing(1);
    let [integrated_local, editable_local] = rewrite_commits_with_change_ids(
        &repo,
        "refs/heads/A",
        &["A~1", "A"],
        &[Some(integrated_change_id.clone()), None],
    )?
    .try_into()
    .expect("rewrote two local commits");
    let [upstream] = rewrite_commits_with_change_ids(
        &repo,
        "refs/remotes/origin/A",
        &["origin/A"],
        &[Some(integrated_change_id)],
    )?
    .try_into()
    .expect("rewrote one upstream commit");
    repo.reference(
        "refs/remotes/origin/main",
        integrated_local,
        gix::refs::transaction::PreviousValue::Any,
        "make rewritten local commit integrated"
            .as_bytes()
            .as_bstr(),
    )?;
    let merge_base = repo.rev_parse_single("A~2")?.detach();

    let initial = initial_integration_for_branch_with_strategy(
        r("refs/heads/A"),
        &repo,
        Some(r("refs/remotes/origin/main")),
        BranchIntegrationStrategy::SmartSquash,
    )?;

    insta::assert_snapshot!(
        labeled_integration_snapshot(
            &initial.integration,
            &[
                (merge_base, "merge-base"),
                (integrated_local, "integrated-local"),
                (editable_local, "editable-local"),
                (upstream, "upstream"),
            ]
        ),
        "smart-squash should not squash into local commits already integrated into the target",
        @"
    merge-base merge-base
    pick upstream
    pick editable-local
    "
    );

    Ok(())
}

#[test]
fn initial_steps_merge_strategy_picks_local_commits_then_merges_upstream_tip() -> Result<()> {
    let mut repo =
        read_only_in_memory_scenario_named("with-remotes-no-workspace", "remote-diverged")?;

    let local_tip = repo.rev_parse_single("A")?.detach();
    let upstream_tip = repo.rev_parse_single("origin/A")?.detach();
    let merge_base = repo.rev_parse_single("A~1")?.detach();
    configure_tracking_for_branch_a(&mut repo)?;

    let initial = initial_integration_for_branch_with_strategy(
        r("refs/heads/A"),
        &repo,
        Some(r("refs/remotes/origin/main")),
        BranchIntegrationStrategy::Merge,
    )?;

    insta::assert_snapshot!(
        labeled_integration_snapshot(
            &initial.integration,
            &[
                (merge_base, "merge-base"),
                (local_tip, "local-tip"),
                (upstream_tip, "upstream-tip"),
            ]
        ),
        @"
    merge-base merge-base
    pick local-tip
    merge upstream-tip
    "
    );

    Ok(())
}

#[test]
fn initial_steps_merge_strategy_skips_integrated_local_commits() -> Result<()> {
    let (_tmp, mut repo) = build_branch_integration_example_repo(
        ExampleScenario::LocalCommitHistoricallyIntegratedOnTarget,
    )?;
    configure_tracking_for_branch_a(&mut repo)?;

    let a = repo.rev_parse_single("main")?.detach();
    let b = repo.rev_parse_single("A~1")?.detach();
    let c = repo.rev_parse_single("A")?.detach();
    let d = repo.rev_parse_single("origin/A")?.detach();

    let initial = initial_integration_for_branch_with_strategy(
        r("refs/heads/A"),
        &repo,
        Some(r("refs/remotes/origin/main")),
        BranchIntegrationStrategy::Merge,
    )?;

    insta::assert_snapshot!(
        labeled_integration_snapshot(&initial.integration, &[(a, "A"), (b, "B"), (c, "C"), (d, "D")]),
        @"
    merge-base A
    pick C
    merge D
    "
    );

    assert!(
        !pick_step_ids(&initial.integration.steps).contains(&b),
        "merge strategy should skip local commits already integrated into the target"
    );

    Ok(())
}

#[test]
fn initial_steps_pick_remote_strategy_picks_only_upstream_commits() -> Result<()> {
    let mut repo = read_only_in_memory_scenario_named(
        "journey03",
        "01-rewritten-local-commit-is-paired-with-remote",
    )?;
    configure_tracking_for_branch_a(&mut repo)?;

    let local_only = repo.rev_parse_single("A~1")?.detach();
    let remote_only = repo.rev_parse_single("origin/A~1")?.detach();
    let local_tip = repo.rev_parse_single("A")?.detach();
    let remote_tip = repo.rev_parse_single("origin/A")?.detach();
    let merge_base = repo.rev_parse_single("A~2")?.detach();

    let initial = initial_integration_for_branch_with_strategy(
        r("refs/heads/A"),
        &repo,
        Some(r("refs/remotes/origin/main")),
        BranchIntegrationStrategy::PickRemote,
    )?;

    insta::assert_snapshot!(
        labeled_integration_snapshot(
            &initial.integration,
            &[
                (merge_base, "merge-base"),
                (local_only, "local-only"),
                (remote_only, "remote-only"),
                (local_tip, "local-tip"),
                (remote_tip, "remote-tip"),
            ]
        ),
        @"
    merge-base merge-base
    pick remote-only
    pick remote-tip
    "
    );

    assert_eq!(
        pick_step_ids(&initial.integration.steps),
        vec![remote_only, remote_tip],
        "pick-remote should keep only upstream commits in parent-to-child order",
    );

    Ok(())
}

#[test]
fn integrate_branch_with_steps_empty_errors_early() -> Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "C", StackState::InWorkspace, &["B"]);
            },
        )?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @r"
    *   f3e1bf2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\
    | * 09d8e52 (A) A
    * | 09bc93e (C) C
    * | c813d8d (B) B
    |/
    * 85efbe4 (origin/main, main) M
    ");

    let mut ws = graph.into_workspace()?;
    let merge_base = repo.rev_parse_single("main")?.detach();
    let integration = InteractiveIntegration {
        merge_base,
        first_local_not_integrated: None,
        steps: vec![],
    };

    configure_tracking_for_branch_a(&mut repo)?;

    let err =
        integrate_branch_with_steps(r("refs/heads/B"), integration, &mut ws, &mut meta, &repo)
            .expect_err("expected early validation error for empty integration steps");
    assert!(
        err.to_string()
            .contains("Integration steps cannot be empty"),
        "unexpected error: {err:#}"
    );

    Ok(())
}

#[test]
fn integrate_branch_with_merge_step_does_not_require_preceding_commit() -> Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "remote-diverged-with-workspace",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            },
        )?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * a7060f8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 8347946 (A) local change in A 2
    * 86838ae local change in A 1
    | * 6a17628 (origin/A) remote change in A 2
    | * 715d7b0 remote change in A 1
    |/
    * 621b98a shared local/remote
    * cfbcc20 (origin/main, main) init-integration
    ");

    let mut ws = graph.into_workspace()?;

    let remote_commit_1 = repo.rev_parse_single("origin/A~1")?.detach();
    let local_commit_1 = repo.rev_parse_single("A~1")?.detach();
    let local_and_remote = repo.rev_parse_single("A~2")?.detach();
    let remote_tip_before = repo.rev_parse_single("origin/A")?.detach();
    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        first_local_not_integrated: Some(local_commit_1),
        steps: vec![InteractiveIntegrationStep::Merge {
            commit_id: remote_commit_1,
        }],
    };

    configure_tracking_for_branch_a(&mut repo)?;

    let rebase =
        integrate_branch_with_steps(r("refs/heads/A"), integration, &mut ws, &mut meta, &repo)?;
    rebase.materialize()?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @r"
    * 1934603 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    *   dc8c3e4 (A) Merge 715d7b0b14844b459ef031a7332283932e99a6a5 into previous commit
    |\
    | | * 6a17628 (origin/A) remote change in A 2
    | |/
    |/|
    * | 715d7b0 remote change in A 1
    |/
    * 621b98a shared local/remote
    * cfbcc20 (origin/main, main) init-integration
    ");

    assert_eq!(
        repo.rev_parse_single("origin/A")?.detach(),
        remote_tip_before
    );

    Ok(())
}

#[test]
fn integrate_upstream_commits_into_local() -> Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "remote-diverged-with-workspace",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            },
        )?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * a7060f8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 8347946 (A) local change in A 2
    * 86838ae local change in A 1
    | * 6a17628 (origin/A) remote change in A 2
    | * 715d7b0 remote change in A 1
    |/
    * 621b98a shared local/remote
    * cfbcc20 (origin/main, main) init-integration
    ");

    let mut ws = graph.into_workspace()?;

    let local_commit_2 = repo.rev_parse_single("A")?.detach();
    let local_commit_1 = repo.rev_parse_single("A~1")?.detach();
    let remote_commit_2 = repo.rev_parse_single("origin/A")?.detach();
    let remote_commit_1 = repo.rev_parse_single("origin/A~1")?.detach();
    let remote_tip_before = remote_commit_2;
    let local_and_remote = repo.rev_parse_single("A~2")?.detach();
    let steps = vec![
        InteractiveIntegrationStep::Pick {
            commit_id: remote_commit_1,
        },
        InteractiveIntegrationStep::Pick {
            commit_id: remote_commit_2,
        },
        InteractiveIntegrationStep::Pick {
            commit_id: local_commit_1,
        },
        InteractiveIntegrationStep::Pick {
            commit_id: local_commit_2,
        },
    ];

    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        first_local_not_integrated: Some(local_commit_1),
        steps,
    };

    configure_tracking_for_branch_a(&mut repo)?;

    let rebase =
        integrate_branch_with_steps(r("refs/heads/A"), integration, &mut ws, &mut meta, &repo)?;
    rebase.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 455d393 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 298d472 (A) local change in A 2
    * 422a07d local change in A 1
    * 6a17628 (origin/A) remote change in A 2
    * 715d7b0 remote change in A 1
    * 621b98a shared local/remote
    * cfbcc20 (origin/main, main) init-integration
    ");

    assert_eq!(
        repo.rev_parse_single("origin/A")?.detach(),
        remote_tip_before
    );

    Ok(())
}

#[test]
fn integrate_upstream_commits_into_local_with_merge_step() -> Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "remote-diverged-with-workspace",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            },
        )?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * a7060f8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 8347946 (A) local change in A 2
    * 86838ae local change in A 1
    | * 6a17628 (origin/A) remote change in A 2
    | * 715d7b0 remote change in A 1
    |/
    * 621b98a shared local/remote
    * cfbcc20 (origin/main, main) init-integration
    ");

    let mut ws = graph.into_workspace()?;

    let local_commit_2 = repo.rev_parse_single("A")?.detach();
    let local_commit_1 = repo.rev_parse_single("A~1")?.detach();
    let remote_commit_1 = repo.rev_parse_single("origin/A~1")?.detach();
    let local_and_remote = repo.rev_parse_single("A~2")?.detach();
    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        first_local_not_integrated: Some(local_commit_1),
        steps: vec![
            InteractiveIntegrationStep::Pick {
                commit_id: local_commit_1,
            },
            InteractiveIntegrationStep::Merge {
                commit_id: remote_commit_1,
            },
            InteractiveIntegrationStep::Pick {
                commit_id: local_commit_2,
            },
        ],
    };

    configure_tracking_for_branch_a(&mut repo)?;

    let rebase =
        integrate_branch_with_steps(r("refs/heads/A"), integration, &mut ws, &mut meta, &repo)?;
    rebase.materialize()?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @r"
    * a74b8e3 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * fdc285b (A) local change in A 2
    *   0d584c5 Merge 715d7b0b14844b459ef031a7332283932e99a6a5 into previous commit
    |\
    * | 86838ae local change in A 1
    | | * 6a17628 (origin/A) remote change in A 2
    | |/
    | * 715d7b0 remote change in A 1
    |/
    * 621b98a shared local/remote
    * cfbcc20 (origin/main, main) init-integration
    ");

    let branch_tip = repo.find_commit(repo.rev_parse_single("A")?.detach())?;
    let branch_tip_parents = branch_tip.parent_ids().collect::<Vec<_>>();
    assert_eq!(
        branch_tip_parents.len(),
        1,
        "tip should remain a non-merge commit"
    );

    let merge_commit_id = branch_tip_parents[0].detach();
    let merge_commit = repo.find_commit(merge_commit_id)?;
    assert_eq!(
        merge_commit.message_raw()?,
        format!("Merge {remote_commit_1} into previous commit")
    );

    let merge_parents = merge_commit.parent_ids().collect::<Vec<_>>();
    assert_eq!(
        merge_parents.len(),
        2,
        "merge step should produce a merge commit"
    );
    assert_eq!(
        merge_parents[1].detach(),
        remote_commit_1,
        "merge step should retain the selected remote commit as the second parent"
    );

    let merged_previous_commit = merge_parents[0].detach();
    let merged_previous = repo.find_commit(merged_previous_commit)?;
    assert_eq!(merged_previous.message_raw()?, "local change in A 1\n");

    insta::assert_snapshot!(visualize_tree(merge_commit.tree_id()?), @"4b825dc");

    Ok(())
}

#[test]
fn integrate_upstream_commits_into_local_with_all_locals_then_merge_second_remote() -> Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "remote-diverged-with-workspace",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            },
        )?;

    let mut ws = graph.into_workspace()?;

    let local_commit_2 = repo.rev_parse_single("A")?.detach();
    let local_commit_1 = repo.rev_parse_single("A~1")?.detach();
    let remote_commit_2 = repo.rev_parse_single("origin/A")?.detach();
    let local_and_remote = repo.rev_parse_single("A~2")?.detach();
    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        first_local_not_integrated: Some(local_commit_1),
        steps: vec![
            InteractiveIntegrationStep::Pick {
                commit_id: local_commit_1,
            },
            InteractiveIntegrationStep::Pick {
                commit_id: local_commit_2,
            },
            InteractiveIntegrationStep::Merge {
                commit_id: remote_commit_2,
            },
        ],
    };

    configure_tracking_for_branch_a(&mut repo)?;

    let rebase =
        integrate_branch_with_steps(r("refs/heads/A"), integration, &mut ws, &mut meta, &repo)?;
    rebase.materialize()?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @r"
    * a11c807 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    *   93bbd52 (A) Merge 6a176285f918d0e4249373b102abe662d4eeeb29 into previous commit
    |\
    | * 6a17628 (origin/A) remote change in A 2
    | * 715d7b0 remote change in A 1
    * | 8347946 local change in A 2
    * | 86838ae local change in A 1
    |/
    * 621b98a shared local/remote
    * cfbcc20 (origin/main, main) init-integration
    ");

    let branch_tip = repo.find_commit(repo.rev_parse_single("A")?.detach())?;
    assert_eq!(
        branch_tip.message_raw()?,
        format!("Merge {remote_commit_2} into previous commit")
    );

    let merge_parents = branch_tip.parent_ids().collect::<Vec<_>>();
    assert_eq!(merge_parents.len(), 2, "tip should be a merge commit");
    assert_eq!(merge_parents[1].detach(), remote_commit_2);

    let first_parent = repo.find_commit(merge_parents[0].detach())?;
    assert_eq!(first_parent.message_raw()?, "local change in A 2\n");

    Ok(())
}

#[test]
fn integrate_upstream_commits_into_local_with_two_merges_in_sequence() -> Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "remote-diverged-with-workspace",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            },
        )?;

    let mut ws = graph.into_workspace()?;

    let local_commit_2 = repo.rev_parse_single("A")?.detach();
    let local_commit_1 = repo.rev_parse_single("A~1")?.detach();
    let remote_commit_1 = repo.rev_parse_single("origin/A~1")?.detach();
    let remote_commit_2 = repo.rev_parse_single("origin/A")?.detach();
    let local_and_remote = repo.rev_parse_single("A~2")?.detach();
    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        first_local_not_integrated: Some(local_commit_1),
        steps: vec![
            InteractiveIntegrationStep::Pick {
                commit_id: local_commit_1,
            },
            InteractiveIntegrationStep::Merge {
                commit_id: remote_commit_1,
            },
            InteractiveIntegrationStep::Pick {
                commit_id: local_commit_2,
            },
            InteractiveIntegrationStep::Merge {
                commit_id: remote_commit_2,
            },
        ],
    };

    configure_tracking_for_branch_a(&mut repo)?;

    let rebase =
        integrate_branch_with_steps(r("refs/heads/A"), integration, &mut ws, &mut meta, &repo)?;
    rebase.materialize()?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @r"
    * d69c4de (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    *   ab7f588 (A) Merge 6a176285f918d0e4249373b102abe662d4eeeb29 into previous commit
    |\
    | * 6a17628 (origin/A) remote change in A 2
    * | fdc285b local change in A 2
    * | 0d584c5 Merge 715d7b0b14844b459ef031a7332283932e99a6a5 into previous commit
    |\|
    | * 715d7b0 remote change in A 1
    * | 86838ae local change in A 1
    |/
    * 621b98a shared local/remote
    * cfbcc20 (origin/main, main) init-integration
    ");

    let branch_tip = repo.find_commit(repo.rev_parse_single("A")?.detach())?;
    assert_eq!(
        branch_tip.message_raw()?,
        format!("Merge {remote_commit_2} into previous commit")
    );

    let branch_tip_parents = branch_tip.parent_ids().collect::<Vec<_>>();
    assert_eq!(branch_tip_parents.len(), 2, "tip should be a merge commit");
    assert_eq!(
        branch_tip_parents[1].detach(),
        remote_commit_2,
        "second merge should keep the selected commit as second parent"
    );

    let first_parent = repo.find_commit(branch_tip_parents[0].detach())?;
    assert_eq!(first_parent.message_raw()?, "local change in A 2\n");
    let first_parent_parents = first_parent.parent_ids().collect::<Vec<_>>();
    assert_eq!(
        first_parent_parents.len(),
        1,
        "the picked local commit before the self-merge should remain linear"
    );
    let remote_merge = repo.find_commit(first_parent_parents[0].detach())?;
    assert_eq!(
        remote_merge.message_raw()?,
        format!("Merge {remote_commit_1} into previous commit"),
        "the later merge should preserve the earlier remote merge in first-parent history"
    );

    Ok(())
}

#[test]
fn integrate_upstream_commits_into_local_with_remote_on_top() -> Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "remote-diverged-with-workspace",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            },
        )?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * a7060f8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 8347946 (A) local change in A 2
    * 86838ae local change in A 1
    | * 6a17628 (origin/A) remote change in A 2
    | * 715d7b0 remote change in A 1
    |/
    * 621b98a shared local/remote
    * cfbcc20 (origin/main, main) init-integration
    ");

    let mut ws = graph.into_workspace()?;

    let local_commit_2 = repo.rev_parse_single("A")?.detach();
    let local_commit_1 = repo.rev_parse_single("A~1")?.detach();
    let remote_commit_2 = repo.rev_parse_single("origin/A")?.detach();
    let remote_commit_1 = repo.rev_parse_single("origin/A~1")?.detach();
    let local_and_remote = repo.rev_parse_single("A~2")?.detach();
    let steps = vec![
        InteractiveIntegrationStep::Pick {
            commit_id: local_commit_1,
        },
        InteractiveIntegrationStep::Pick {
            commit_id: local_commit_2,
        },
        InteractiveIntegrationStep::Pick {
            commit_id: remote_commit_1,
        },
        InteractiveIntegrationStep::Pick {
            commit_id: remote_commit_2,
        },
    ];

    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        first_local_not_integrated: Some(local_commit_1),
        steps,
    };

    configure_tracking_for_branch_a(&mut repo)?;

    let rebase =
        integrate_branch_with_steps(r("refs/heads/A"), integration, &mut ws, &mut meta, &repo)?;
    rebase.materialize()?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * fb437fd (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 85ce57b (A) remote change in A 2
    * 01b7a91 remote change in A 1
    * 8347946 local change in A 2
    * 86838ae local change in A 1
    | * 6a17628 (origin/A) remote change in A 2
    | * 715d7b0 remote change in A 1
    |/
    * 621b98a shared local/remote
    * cfbcc20 (origin/main, main) init-integration
    ");

    Ok(())
}

#[test]
fn integrate_upstream_commits_into_local_with_remote_interlaced() -> Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "remote-diverged-with-workspace",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            },
        )?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * a7060f8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 8347946 (A) local change in A 2
    * 86838ae local change in A 1
    | * 6a17628 (origin/A) remote change in A 2
    | * 715d7b0 remote change in A 1
    |/
    * 621b98a shared local/remote
    * cfbcc20 (origin/main, main) init-integration
    ");

    let mut ws = graph.into_workspace()?;

    let local_commit_2 = repo.rev_parse_single("A")?.detach();
    let local_commit_1 = repo.rev_parse_single("A~1")?.detach();
    let remote_commit_2 = repo.rev_parse_single("origin/A")?.detach();
    let remote_commit_1 = repo.rev_parse_single("origin/A~1")?.detach();
    let local_and_remote = repo.rev_parse_single("A~2")?.detach();
    let steps = vec![
        InteractiveIntegrationStep::Pick {
            commit_id: remote_commit_1,
        },
        InteractiveIntegrationStep::Pick {
            commit_id: local_commit_1,
        },
        InteractiveIntegrationStep::Pick {
            commit_id: remote_commit_2,
        },
        InteractiveIntegrationStep::Pick {
            commit_id: local_commit_2,
        },
    ];

    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        first_local_not_integrated: Some(local_commit_1),
        steps,
    };

    configure_tracking_for_branch_a(&mut repo)?;

    let rebase =
        integrate_branch_with_steps(r("refs/heads/A"), integration, &mut ws, &mut meta, &repo)?;
    rebase.materialize()?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * 0ce7098 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * ad12639 (A) local change in A 2
    * a6a4994 remote change in A 2
    * 593d2d6 local change in A 1
    | * 6a17628 (origin/A) remote change in A 2
    |/
    * 715d7b0 remote change in A 1
    * 621b98a shared local/remote
    * cfbcc20 (origin/main, main) init-integration
    ");

    Ok(())
}

#[test]
fn integrate_upstream_commits_into_local_with_remote_one_local_one_remote() -> Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "remote-diverged-with-workspace",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            },
        )?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * a7060f8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 8347946 (A) local change in A 2
    * 86838ae local change in A 1
    | * 6a17628 (origin/A) remote change in A 2
    | * 715d7b0 remote change in A 1
    |/
    * 621b98a shared local/remote
    * cfbcc20 (origin/main, main) init-integration
    ");

    let mut ws = graph.into_workspace()?;

    let local_commit_1 = repo.rev_parse_single("A~1")?.detach();
    let remote_commit_2 = repo.rev_parse_single("origin/A")?.detach();
    let local_and_remote = repo.rev_parse_single("A~2")?.detach();
    let steps = vec![
        InteractiveIntegrationStep::Pick {
            commit_id: local_commit_1,
        },
        InteractiveIntegrationStep::Pick {
            commit_id: remote_commit_2,
        },
    ];

    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        first_local_not_integrated: Some(local_commit_1),
        steps,
    };

    configure_tracking_for_branch_a(&mut repo)?;

    let rebase =
        integrate_branch_with_steps(r("refs/heads/A"), integration, &mut ws, &mut meta, &repo)?;
    rebase.materialize()?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * ab8c010 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 801c92f (A) remote change in A 2
    * 86838ae local change in A 1
    | * 6a17628 (origin/A) remote change in A 2
    | * 715d7b0 remote change in A 1
    |/
    * 621b98a shared local/remote
    * cfbcc20 (origin/main, main) init-integration
    ");

    Ok(())
}

#[test]
fn integrate_upstream_commits_into_local_with_remote_one_local_one_remote_and_extra_local_ref()
-> Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "remote-diverged-with-workspace",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            },
        )?;

    add_local_ref_at_ref(&repo, "A-shadow", "A")?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * a7060f8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 8347946 (A-shadow, A) local change in A 2
    * 86838ae local change in A 1
    | * 6a17628 (origin/A) remote change in A 2
    | * 715d7b0 remote change in A 1
    |/
    * 621b98a shared local/remote
    * cfbcc20 (origin/main, main) init-integration
    ");

    let mut ws = graph.into_workspace()?;

    let local_commit_1 = repo.rev_parse_single("A~1")?.detach();
    let remote_commit_2 = repo.rev_parse_single("origin/A")?.detach();
    let local_and_remote = repo.rev_parse_single("A~2")?.detach();
    let steps = vec![
        InteractiveIntegrationStep::Pick {
            commit_id: local_commit_1,
        },
        InteractiveIntegrationStep::Pick {
            commit_id: remote_commit_2,
        },
    ];

    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        first_local_not_integrated: Some(local_commit_1),
        steps,
    };

    configure_tracking_for_branch_a(&mut repo)?;

    let rebase =
        integrate_branch_with_steps(r("refs/heads/A"), integration, &mut ws, &mut meta, &repo)?;
    rebase.materialize()?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * 8347946 (A-shadow) local change in A 2
    | * ab8c010 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * 801c92f (A) remote change in A 2
    |/
    * 86838ae local change in A 1
    | * 6a17628 (origin/A) remote change in A 2
    | * 715d7b0 remote change in A 1
    |/
    * 621b98a shared local/remote
    * cfbcc20 (origin/main, main) init-integration
    ");

    Ok(())
}

#[test]
fn integrate_upstream_commits_into_local_with_only_remote_commits() -> Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "remote-diverged-with-workspace",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            },
        )?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * a7060f8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 8347946 (A) local change in A 2
    * 86838ae local change in A 1
    | * 6a17628 (origin/A) remote change in A 2
    | * 715d7b0 remote change in A 1
    |/
    * 621b98a shared local/remote
    * cfbcc20 (origin/main, main) init-integration
    ");

    let mut ws = graph.into_workspace()?;

    let remote_commit_2 = repo.rev_parse_single("origin/A")?.detach();
    let remote_commit_1 = repo.rev_parse_single("origin/A~1")?.detach();
    let local_commit_1 = repo.rev_parse_single("A~1")?.detach();
    let local_and_remote = repo.rev_parse_single("A~2")?.detach();
    let steps = vec![
        InteractiveIntegrationStep::Pick {
            commit_id: remote_commit_1,
        },
        InteractiveIntegrationStep::Pick {
            commit_id: remote_commit_2,
        },
    ];

    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        first_local_not_integrated: Some(local_commit_1),
        steps,
    };

    configure_tracking_for_branch_a(&mut repo)?;

    let rebase =
        integrate_branch_with_steps(r("refs/heads/A"), integration, &mut ws, &mut meta, &repo)?;
    rebase.materialize()?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * b3d4566 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 6a17628 (origin/A, A) remote change in A 2
    * 715d7b0 remote change in A 1
    * 621b98a shared local/remote
    * cfbcc20 (origin/main, main) init-integration
    ");

    Ok(())
}

#[test]
fn integrate_upstream_commits_into_local_with_squashed_local_commits() -> Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "remote-diverged-with-workspace",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            },
        )?;

    let mut ws = graph.into_workspace()?;

    let local_commit_2 = repo.rev_parse_single("A")?.detach();
    let local_commit_1 = repo.rev_parse_single("A~1")?.detach();
    let remote_commit_2 = repo.rev_parse_single("origin/A")?.detach();
    let remote_commit_1 = repo.rev_parse_single("origin/A~1")?.detach();
    let local_and_remote = repo.rev_parse_single("A~2")?.detach();
    let steps = vec![
        InteractiveIntegrationStep::Pick {
            commit_id: remote_commit_1,
        },
        InteractiveIntegrationStep::Pick {
            commit_id: remote_commit_2,
        },
        InteractiveIntegrationStep::Squash {
            commits: vec![local_commit_1, local_commit_2],
            message: Some("squashed local commits".to_string()),
        },
    ];

    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        first_local_not_integrated: Some(local_commit_1),
        steps,
    };

    configure_tracking_for_branch_a(&mut repo)?;

    let rebase =
        integrate_branch_with_steps(r("refs/heads/A"), integration, &mut ws, &mut meta, &repo)?;
    rebase.materialize()?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * 5ef31c2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * c297225 (A) squashed local commits
    * 6a17628 (origin/A) remote change in A 2
    * 715d7b0 remote change in A 1
    * 621b98a shared local/remote
    * cfbcc20 (origin/main, main) init-integration
    ");

    let branch_tip = repo.find_commit(repo.rev_parse_single("A")?.detach())?;
    assert_eq!(branch_tip.message_raw()?, "squashed local commits");

    Ok(())
}

#[test]
fn integrate_upstream_commits_into_local_with_squashed_remote_commits() -> Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "remote-diverged-with-workspace",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            },
        )?;

    let mut ws = graph.into_workspace()?;

    let local_commit_2 = repo.rev_parse_single("A")?.detach();
    let local_commit_1 = repo.rev_parse_single("A~1")?.detach();
    let remote_commit_2 = repo.rev_parse_single("origin/A")?.detach();
    let remote_commit_1 = repo.rev_parse_single("origin/A~1")?.detach();
    let local_and_remote = repo.rev_parse_single("A~2")?.detach();
    let steps = vec![
        InteractiveIntegrationStep::Pick {
            commit_id: local_commit_1,
        },
        InteractiveIntegrationStep::Pick {
            commit_id: local_commit_2,
        },
        InteractiveIntegrationStep::Squash {
            commits: vec![remote_commit_1, remote_commit_2],
            message: Some("squashed remote commits".to_string()),
        },
    ];

    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        first_local_not_integrated: Some(local_commit_1),
        steps,
    };

    configure_tracking_for_branch_a(&mut repo)?;

    let rebase =
        integrate_branch_with_steps(r("refs/heads/A"), integration, &mut ws, &mut meta, &repo)?;
    rebase.materialize()?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * 3699070 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 3838b79 (A) squashed remote commits
    * 8347946 local change in A 2
    * 86838ae local change in A 1
    | * 6a17628 (origin/A) remote change in A 2
    | * 715d7b0 remote change in A 1
    |/
    * 621b98a shared local/remote
    * cfbcc20 (origin/main, main) init-integration
    ");

    let branch_tip = repo.find_commit(repo.rev_parse_single("A")?.detach())?;
    assert_eq!(branch_tip.message_raw()?, "squashed remote commits");

    Ok(())
}

#[test]
fn integrate_upstream_commits_into_local_with_squashed_remote_into_local_commits() -> Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "remote-diverged-with-workspace",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            },
        )?;

    let mut ws = graph.into_workspace()?;

    let local_commit_2 = repo.rev_parse_single("A")?.detach();
    let local_commit_1 = repo.rev_parse_single("A~1")?.detach();
    let remote_commit_2 = repo.rev_parse_single("origin/A")?.detach();
    let remote_commit_1 = repo.rev_parse_single("origin/A~1")?.detach();
    let local_and_remote = repo.rev_parse_single("A~2")?.detach();
    let steps = vec![
        InteractiveIntegrationStep::Squash {
            commits: vec![remote_commit_1, local_commit_1],
            message: Some("squash commits 1".to_string()),
        },
        InteractiveIntegrationStep::Squash {
            commits: vec![remote_commit_2, local_commit_2],
            message: Some("squash commits 2".to_string()),
        },
    ];

    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        first_local_not_integrated: Some(local_commit_1),
        steps,
    };

    configure_tracking_for_branch_a(&mut repo)?;

    let rebase =
        integrate_branch_with_steps(r("refs/heads/A"), integration, &mut ws, &mut meta, &repo)?;
    rebase.materialize()?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * 8a9dd44 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * c6b942b (A) squash commits 2
    * a524f0a squash commits 1
    | * 6a17628 (origin/A) remote change in A 2
    | * 715d7b0 remote change in A 1
    |/
    * 621b98a shared local/remote
    * cfbcc20 (origin/main, main) init-integration
    ");

    Ok(())
}

#[test]
fn integrate_upstream_commits_into_local_with_squashed_remote_into_local_conflicts() -> Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "remote-diverged-with-workspace-conflicting",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            },
        )?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * 8fd8fb6 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 61c4a24 (A) local change in A 1
    | * f03fc2c (origin/A, new-origin) remote change in A 1
    |/
    * 2b73dee (origin/main, main) init-integration
    ");

    let mut ws = graph.into_workspace()?;

    let local_commit_1 = repo.rev_parse_single("A")?.detach();
    let remote_commit_1 = repo.rev_parse_single("origin/A")?.detach();
    let local_and_remote = repo.rev_parse_single("main")?.detach();
    let steps = vec![InteractiveIntegrationStep::Squash {
        commits: vec![remote_commit_1, local_commit_1],
        message: Some("squashed conflicting commits".to_string()),
    }];

    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        first_local_not_integrated: Some(local_commit_1),
        steps,
    };

    configure_tracking_for_branch_a(&mut repo)?;

    let rebase =
        integrate_branch_with_steps(r("refs/heads/A"), integration, &mut ws, &mut meta, &repo)?;
    rebase.materialize()?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * f03fc2c (origin/A, new-origin) remote change in A 1
    | * 1b052b4 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * 20ebfcc (A) [conflict] squashed conflicting commits
    |/
    * 2b73dee (origin/main, main) init-integration
    ");

    let branch_tip = repo.find_commit(repo.rev_parse_single("A")?.detach())?;
    assert!(Commit::from_id(branch_tip.id.attach(&repo))?.is_conflicted());
    insta::assert_snapshot!(branch_tip.message_raw()?, @r#"
    [conflict] squashed conflicting commits

    GitButler-Conflict: This is a GitButler-managed conflicted commit. Files are auto-resolved
       using the "ours" side. The commit tree contains additional directories:
         .conflict-side-0  — our tree
         .conflict-side-1  — their tree
         .conflict-base-0  — the merge base tree
         .auto-resolution  — the auto-resolved tree
         .conflict-files   — metadata about conflicted files
       To manually resolve, check out this commit, remove the directories
       listed above, resolve the conflicts, and amend the commit.
    "#);
    insta::assert_snapshot!(normalized_tree_snapshot(branch_tip.tree_id()?.detach(), &repo)?, @r#"
    450d676
    ├── .auto-resolution:276d2b4
    │   └── shared.txt:100644:4083037 "local\n"
    ├── .conflict-base-0:48e531d
    │   └── shared.txt:100644:df967b9 "base\n"
    ├── .conflict-files:100644:d0a3da4 "ancestorEntries = [\"shared.txt\"]\nourEntries = [\"shared.txt\"]\ntheirEntries = [\"shared.txt\"]\n"
    ├── .conflict-side-0:276d2b4
    │   └── shared.txt:100644:4083037 "local\n"
    ├── .conflict-side-1:cd74779
    │   └── shared.txt:100644:9c998f7 "remote\n"
    └── shared.txt:100644:4083037 "local\n"
    "#);

    Ok(())
}

#[test]
fn integrate_upstream_commits_into_local_with_merge_remote_into_local_conflicts() -> Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "remote-diverged-with-workspace-conflicting",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            },
        )?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * 8fd8fb6 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 61c4a24 (A) local change in A 1
    | * f03fc2c (origin/A, new-origin) remote change in A 1
    |/
    * 2b73dee (origin/main, main) init-integration
    ");

    let mut ws = graph.into_workspace()?;

    let local_commit_1 = repo.rev_parse_single("A")?.detach();
    let remote_commit_1 = repo.rev_parse_single("origin/A")?.detach();
    let local_and_remote = repo.rev_parse_single("main")?.detach();
    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        first_local_not_integrated: Some(local_commit_1),
        steps: vec![
            InteractiveIntegrationStep::Pick {
                commit_id: local_commit_1,
            },
            InteractiveIntegrationStep::Merge {
                commit_id: remote_commit_1,
            },
        ],
    };

    configure_tracking_for_branch_a(&mut repo)?;

    let rebase =
        integrate_branch_with_steps(r("refs/heads/A"), integration, &mut ws, &mut meta, &repo)?;
    rebase.materialize()?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @r"
    * 9b44771 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    *   c813ff0 (A) [conflict] Merge f03fc2cb7251fb1707fa0f7bee28eb507ec1405c into previous commit
    |\
    | * f03fc2c (origin/A, new-origin) remote change in A 1
    * | 61c4a24 local change in A 1
    |/
    * 2b73dee (origin/main, main) init-integration
    ");

    let branch_tip = repo.find_commit(repo.rev_parse_single("A")?.detach())?;
    assert!(
        Commit::from_id(branch_tip.id.attach(&repo))?.is_conflicted(),
        "merge integration should materialize a conflicted commit when upstream and local changes conflict",
    );
    assert_eq!(
        branch_tip.message_raw()?,
        format!(
            "[conflict] Merge {remote_commit_1} into previous commit\n\nGitButler-Conflict: This is a GitButler-managed conflicted commit. Files are auto-resolved\n   using the \"ours\" side. The commit tree contains additional directories:\n     .conflict-side-0  — our tree\n     .conflict-side-1  — their tree\n     .conflict-base-0  — the merge base tree\n     .auto-resolution  — the auto-resolved tree\n     .conflict-files   — metadata about conflicted files\n   To manually resolve, check out this commit, remove the directories\n   listed above, resolve the conflicts, and amend the commit.\n"
        ),
        "merge integration should write the standard conflicted-commit message",
    );

    let merge_parents = branch_tip.parent_ids().collect::<Vec<_>>();
    assert_eq!(
        merge_parents.len(),
        2,
        "conflicted merge integration should still produce a merge commit",
    );
    assert_eq!(
        merge_parents[1].detach(),
        remote_commit_1,
        "merge integration should retain the upstream commit as the second parent even when conflicted",
    );

    let first_parent = repo.find_commit(merge_parents[0].detach())?;
    assert_eq!(
        first_parent.message_raw()?,
        "local change in A 1\n",
        "merge integration should retain the local commit as first parent even when conflicted",
    );

    insta::assert_snapshot!(normalized_tree_snapshot(branch_tip.tree_id()?.detach(), &repo)?, @r#"
    450d676
    ├── .auto-resolution:276d2b4
    │   └── shared.txt:100644:4083037 "local\n"
    ├── .conflict-base-0:48e531d
    │   └── shared.txt:100644:df967b9 "base\n"
    ├── .conflict-files:100644:d0a3da4 "ancestorEntries = [\"shared.txt\"]\nourEntries = [\"shared.txt\"]\ntheirEntries = [\"shared.txt\"]\n"
    ├── .conflict-side-0:276d2b4
    │   └── shared.txt:100644:4083037 "local\n"
    ├── .conflict-side-1:cd74779
    │   └── shared.txt:100644:9c998f7 "remote\n"
    └── shared.txt:100644:4083037 "local\n"
    "#);

    Ok(())
}

#[test]
fn integrate_upstream_commits_into_local_with_merge_remote_into_local_conflicts_preview()
-> Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "remote-diverged-with-workspace-conflicting",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            },
        )?;

    let mut ws = graph.into_workspace()?;

    let local_commit_1 = repo.rev_parse_single("A")?.detach();
    let remote_commit_1 = repo.rev_parse_single("origin/A")?.detach();
    let local_and_remote = repo.rev_parse_single("main")?.detach();
    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        first_local_not_integrated: Some(local_commit_1),
        steps: vec![
            InteractiveIntegrationStep::Pick {
                commit_id: local_commit_1,
            },
            InteractiveIntegrationStep::Merge {
                commit_id: remote_commit_1,
            },
        ],
    };

    configure_tracking_for_branch_a(&mut repo)?;

    let rebase =
        integrate_branch_with_steps(r("refs/heads/A"), integration, &mut ws, &mut meta, &repo)?;
    let preview_graph = rebase.overlayed_graph()?;
    let preview_workspace = preview_graph.into_workspace()?;
    let ref_info = but_workspace::graph_to_ref_info(
        &preview_workspace,
        rebase.repo(),
        but_workspace::ref_info::Options {
            traversal: but_graph::init::Options::limited(),
            expensive_commit_info: true,
            ..Default::default()
        },
    )?
    .pruned_to_entrypoint();

    assert!(
        !ref_info.stacks.is_empty(),
        "dry-run branch integration preview should produce stack information"
    );

    Ok(())
}

#[test]
fn integrate_upstream_precomputes_squash_before_later_step_graph_rewiring() -> Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "remote-diverged-with-workspace",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            },
        )?;

    let mut ws = graph.into_workspace()?;

    let local_commit_2 = repo.rev_parse_single("A")?.detach();
    let local_commit_1 = repo.rev_parse_single("A~1")?.detach();
    let local_and_remote = repo.rev_parse_single("A~2")?.detach();
    let expected_squash_tree = repo.find_commit(local_commit_2)?.tree_id()?.detach();
    let steps = vec![
        InteractiveIntegrationStep::Squash {
            commits: vec![local_commit_1, local_commit_2],
            message: Some("squashed local commits".to_string()),
        },
        InteractiveIntegrationStep::Pick {
            commit_id: local_commit_2,
        },
    ];

    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        first_local_not_integrated: Some(local_commit_1),
        steps,
    };

    configure_tracking_for_branch_a(&mut repo)?;

    let rebase =
        integrate_branch_with_steps(r("refs/heads/A"), integration, &mut ws, &mut meta, &repo)?;
    rebase.materialize()?;

    let mut current_commit_id = repo.rev_parse_single("A")?.detach();
    let squashed_commit_id = loop {
        let commit = repo.find_commit(current_commit_id)?;
        if commit.message_raw()? == "squashed local commits" {
            break current_commit_id;
        }
        let Some(parent_id) = commit.parent_ids().next() else {
            panic!("prepared squash should still be materialized into first-parent history");
        };
        current_commit_id = parent_id.detach();
    };
    let squashed_commit = repo.find_commit(squashed_commit_id)?;
    assert_eq!(
        squashed_commit.tree_id()?.detach(),
        expected_squash_tree,
        "squash tree should be computed from the original repo topology before later graph rewiring",
    );

    Ok(())
}

#[test]
fn initial_steps_remote_diverged_with_workspace_are_in_application_order() -> Result<()> {
    let (_tmp, _graph, mut repo, mut _meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "remote-diverged-with-workspace",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            },
        )?;
    configure_tracking_for_branch_a(&mut repo)?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * a7060f8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 8347946 (A) local change in A 2
    * 86838ae local change in A 1
    | * 6a17628 (origin/A) remote change in A 2
    | * 715d7b0 remote change in A 1
    |/
    * 621b98a shared local/remote
    * cfbcc20 (origin/main, main) init-integration
    ");

    let local_commit_2 = repo.rev_parse_single("A")?.detach();
    let local_commit_1 = repo.rev_parse_single("A~1")?.detach();
    let remote_commit_2 = repo.rev_parse_single("origin/A")?.detach();
    let remote_commit_1 = repo.rev_parse_single("origin/A~1")?.detach();
    let merge_base = repo.rev_parse_single("A~2")?.detach();
    configure_tracking_for_branch_a(&mut repo)?;

    let initial = initial_integration_for_branch(
        r("refs/heads/A"),
        &repo,
        Some(r("refs/remotes/origin/main")),
    )?;

    insta::assert_snapshot!(
        labeled_integration_snapshot(
            &initial.integration,
            &[
                (merge_base, "merge-base"),
                (local_commit_2, "local-commit-2"),
                (local_commit_1, "local-commit-1"),
                (remote_commit_2, "remote-commit-2"),
                (remote_commit_1, "remote-commit-1"),
            ]
        ),
        @"
    merge-base merge-base
    pick remote-commit-1
    pick remote-commit-2
    pick local-commit-1
    pick local-commit-2
    "
    );

    insta::assert_snapshot!(
        labeled_divergence_snapshot(
            &initial,
            &[
                (merge_base, "merge-base"),
                (local_commit_2, "local-commit-2"),
                (local_commit_1, "local-commit-1"),
                (remote_commit_2, "remote-commit-2"),
                (remote_commit_1, "remote-commit-1"),
            ]
        ),
        @"
        * local-commit-2 (A) local change in A 2
        * local-commit-1 local change in A 1
        | * remote-commit-2 (origin/A) remote change in A 2
        | * remote-commit-1 remote change in A 1
        |/
        * merge-base shared local/remote
        "
    );

    assert_eq!(
        pick_step_ids(&initial.integration.steps),
        vec![
            remote_commit_1,
            remote_commit_2,
            local_commit_1,
            local_commit_2
        ],
        "expected parent-to-child application order for the integrated branch"
    );

    Ok(())
}

#[test]
fn initial_steps_example_1_keep_integrated_target_history_out_of_divergence() -> Result<()> {
    let (_tmp, mut repo) = build_branch_integration_example_repo(
        ExampleScenario::ExtraTargetHistoryExcludedFromDivergence,
    )?;
    configure_tracking_for_branch_a(&mut repo)?;

    let b = repo.rev_parse_single("main")?.detach();
    let c = repo.rev_parse_single("A")?.detach();
    let d = repo.rev_parse_single("origin/A")?.detach();
    let x = repo.rev_parse_single("origin/main")?.detach();

    let initial = initial_integration_for_branch(
        r("refs/heads/A"),
        &repo,
        Some(r("refs/remotes/origin/main")),
    )?;

    insta::assert_snapshot!(
        labeled_divergence_snapshot(&initial, &[(b, "B"), (c, "C"), (d, "D"), (x, "X")]),
        @"
        * C (A) C
        | * D (origin/A) D
        |/
        * B [historically-integrated:B] B
        "
    );
    insta::assert_snapshot!(
        labeled_integration_snapshot(&initial.integration, &[(b, "B"), (c, "C"), (d, "D")]),
        @"
    merge-base B
    pick D
    pick C
    "
    );

    assert_eq!(
        initial.integration.merge_base, b,
        "example 1 merge-base should remain B"
    );
    assert_eq!(
        pick_step_ids(&initial.integration.steps),
        vec![d, c],
        "example 1 should keep upstream then local editable commits in application order",
    );
    assert_eq!(
        initial
            .divergence
            .local_only
            .iter()
            .map(|commit| commit.id)
            .collect::<Vec<_>>(),
        vec![c],
        "example 1 should only show the local tip above the merge-base",
    );
    assert_eq!(
        initial
            .divergence
            .upstream_only
            .iter()
            .map(|commit| commit.id)
            .collect::<Vec<_>>(),
        vec![d],
        "example 1 should only show the upstream tip above the merge-base",
    );
    assert!(
        !initial
            .divergence
            .local_only
            .iter()
            .chain(initial.divergence.upstream_only.iter())
            .any(|commit| commit.id == x),
        "example 1 keeps target-only history out of divergence rows",
    );

    Ok(())
}

#[test]
fn initial_steps_example_2_excludes_integrated_local_commits_but_keeps_them_visible() -> Result<()>
{
    let (_tmp, mut repo) = build_branch_integration_example_repo(
        ExampleScenario::LocalCommitHistoricallyIntegratedOnTarget,
    )?;
    configure_tracking_for_branch_a(&mut repo)?;

    let a = repo.rev_parse_single("main")?.detach();
    let b = repo.rev_parse_single("A~1")?.detach();
    let c = repo.rev_parse_single("A")?.detach();
    let d = repo.rev_parse_single("origin/A")?.detach();

    let initial = initial_integration_for_branch(
        r("refs/heads/A"),
        &repo,
        Some(r("refs/remotes/origin/main")),
    )?;

    assert_eq!(
        initial.integration.merge_base, a,
        "example 2 merge-base should remain A"
    );
    assert_eq!(
        initial.integration.first_local_not_integrated,
        Some(c),
        "example 2 should record the first editable local commit as execution metadata",
    );
    // Pick only commit D and commit C.
    assert_eq!(
        pick_step_ids(&initial.integration.steps),
        vec![d, c],
        "example 2 should skip integrated local B while keeping local C editable",
    );
    insta::assert_snapshot!(
        labeled_divergence_snapshot(
            &initial,
            &[(a, "A"), (b, "B"), (c, "C"), (d, "D")]
        ),
        @"
        * C (A) C
        * B [historically-integrated:B] B
        | * D (origin/A) D
        |/
        * A [historically-integrated:A] A
        "
    );
    assert_eq!(
        initial.divergence.local_only[1].target_relation,
        IntegrationDivergenceTargetRelation::HistoricallyIntegrated {
            target_commit_id: b
        },
        "example 2 should annotate integrated local B with the target commit that contains it",
    );

    Ok(())
}

#[test]
fn apply_initial_steps_example_2_also_applies_integrated_local_commits() -> Result<()> {
    let (_tmp, mut repo) = build_branch_integration_example_repo(
        ExampleScenario::LocalCommitHistoricallyIntegratedOnTarget,
    )?;
    configure_tracking_for_branch_a(&mut repo)?;
    let a = repo.rev_parse_single("main")?.detach();
    let b = repo.rev_parse_single("A~1")?.detach();
    let c = repo.rev_parse_single("A")?.detach();
    let d = repo.rev_parse_single("origin/A")?.detach();
    let x = repo.rev_parse_single("origin/main")?.detach();

    let initial = initial_integration_for_branch(
        r("refs/heads/A"),
        &repo,
        Some(r("refs/remotes/origin/main")),
    )?;
    assert_eq!(
        initial.integration.first_local_not_integrated,
        Some(c),
        "example 2 should pick c as the first non-integrated local commit"
    );
    assert_eq!(
        pick_step_ids(&initial.integration.steps),
        vec![d, c],
        "example 2 should still omit integrated local commits from the editable plan"
    );

    let (mut workspace, mut meta) = integration_workspace_for_branch(
        r("refs/heads/A"),
        &repo,
        Some(r("refs/remotes/origin/main")),
    )?;
    let rebase = integrate_branch_with_steps(
        r("refs/heads/A"),
        initial.integration,
        &mut workspace,
        &mut meta,
        &repo,
    )?;

    rebase.materialize()?;
    let c_prime = repo.rev_parse_single("A")?.detach();
    let d_prime = repo.rev_parse_single("A~1")?.detach();

    insta::assert_snapshot!(
        labeled_graph_snapshot(
            &repo,
            &[
                (a, "A"),
                (b, "B"),
                (d, "D"),
                (x, "X"),
                (d_prime, "D'"),
                (c_prime, "C'"),
            ]
        )?,
        @"
    * C' (HEAD -> A) C
    * D' [conflict] D
    | * X (origin/main, target-main) X
    |/
    * B B
    | * D (origin/A, upstream-a) D
    |/
    * A (main) A
    "
    );

    Ok(())
}

#[test]
fn initial_steps_example_3_keeps_integrated_upstream_commits_editable() -> Result<()> {
    let (_tmp, mut repo) = build_branch_integration_example_repo(
        ExampleScenario::UpstreamCommitHistoricallyIntegratedOnTarget,
    )?;
    configure_tracking_for_branch_a(&mut repo)?;

    let b = repo.rev_parse_single("A~1")?.detach();
    let c = repo.rev_parse_single("A")?.detach();
    let x = repo.rev_parse_single("origin/A~1")?.detach();
    let d = repo.rev_parse_single("origin/A")?.detach();

    let initial = initial_integration_for_branch(
        r("refs/heads/A"),
        &repo,
        Some(r("refs/remotes/origin/main")),
    )?;

    assert_eq!(
        initial.integration.merge_base, b,
        "example 3 merge-base should remain B"
    );
    // Pick X, D and C
    assert_eq!(
        pick_step_ids(&initial.integration.steps),
        vec![x, d, c],
        "example 3 should keep integrated upstream X editable before D and local C",
    );
    insta::assert_snapshot!(
        labeled_divergence_snapshot(&initial, &[(b, "B"), (c, "C"), (d, "D"), (x, "X")]),
        @"
        * C (A) C
        | * D (origin/A) D
        | * X [historically-integrated:X] X
        |/
        * B [historically-integrated:B] B
        "
    );

    Ok(())
}

fn configure_tracking_for_branch_a(repo: &mut gix::Repository) -> Result<()> {
    let mut cfg = repo.config_snapshot_mut();
    cfg.set_raw_value(
        "remote.origin.fetch",
        gix::bstr::BStr::new(b"+refs/heads/*:refs/remotes/origin/*"),
    )?;
    cfg.set_raw_value("remote.origin.url", gix::bstr::BStr::new(b"."))?;
    cfg.set_raw_value("branch.A.remote", gix::bstr::BStr::new(b"origin"))?;
    cfg.set_raw_value("branch.A.merge", gix::bstr::BStr::new(b"refs/heads/A"))?;
    Ok(())
}

fn pick_step_ids(steps: &[InteractiveIntegrationStep]) -> Vec<gix::ObjectId> {
    steps
        .iter()
        .map(|step| match step {
            InteractiveIntegrationStep::Pick { commit_id, .. }
            | InteractiveIntegrationStep::Merge { commit_id, .. } => *commit_id,
            InteractiveIntegrationStep::Squash { commits, .. } => {
                *commits.last().expect("squash step should contain commits")
            }
        })
        .collect()
}

fn add_local_ref_at_ref(repo: &gix::Repository, new_branch: &str, target: &str) -> Result<()> {
    let workdir = repo.workdir().expect("writable scenarios are non-bare");
    let target_id = repo.rev_parse_single(target)?.detach();

    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(workdir)
        .arg("update-ref")
        .arg(format!("refs/heads/{new_branch}"))
        .arg(target_id.to_string())
        .status()?;

    if !status.success() {
        bail!("failed to create local reference refs/heads/{new_branch}");
    }

    Ok(())
}

#[derive(Clone, Copy)]
enum ExampleScenario {
    ExtraTargetHistoryExcludedFromDivergence,
    LocalCommitHistoricallyIntegratedOnTarget,
    UpstreamCommitHistoricallyIntegratedOnTarget,
}

fn build_branch_integration_example_repo(
    scenario: ExampleScenario,
) -> Result<(TempDir, gix::Repository)> {
    let tmp = TempDir::new()?;
    let repo_dir = tmp.path().to_path_buf();

    run_git(&repo_dir, &["init", "--initial-branch=main"])?;
    run_git(&repo_dir, &["config", "user.name", "GitButler Tests"])?;
    run_git(&repo_dir, &["config", "user.email", "tests@gitbutler.com"])?;
    run_git(&repo_dir, &["config", "commit.gpgsign", "false"])?;

    write_file(&repo_dir, "story.txt", "A\n")?;
    run_git(&repo_dir, &["add", "story.txt"])?;
    run_git(&repo_dir, &["commit", "-m", "A"])?;
    let a = git_rev_parse(&repo_dir, "HEAD")?;

    match scenario {
        ExampleScenario::ExtraTargetHistoryExcludedFromDivergence => {
            append_and_commit(&repo_dir, "story.txt", "B\n", "B")?;
            let b = git_rev_parse(&repo_dir, "HEAD")?;

            run_git(&repo_dir, &["checkout", "-b", "A", &b])?;
            append_and_commit(&repo_dir, "story.txt", "C\n", "C")?;

            run_git(&repo_dir, &["checkout", "-b", "target-main", &b])?;
            append_and_commit(&repo_dir, "story.txt", "X\n", "X")?;

            run_git(&repo_dir, &["checkout", "-b", "upstream-a", &b])?;
            append_and_commit(&repo_dir, "story.txt", "D\n", "D")?;
        }
        ExampleScenario::LocalCommitHistoricallyIntegratedOnTarget => {
            run_git(&repo_dir, &["checkout", "-b", "A", &a])?;
            append_and_commit(&repo_dir, "story.txt", "B\n", "B")?;
            let b = git_rev_parse(&repo_dir, "HEAD")?;
            append_and_commit(&repo_dir, "story.txt", "C\n", "C")?;

            run_git(&repo_dir, &["checkout", "-b", "target-main", &b])?;
            append_and_commit(&repo_dir, "story.txt", "X\n", "X")?;

            run_git(&repo_dir, &["checkout", "-b", "upstream-a", &a])?;
            append_and_commit(&repo_dir, "story.txt", "D\n", "D")?;
        }
        ExampleScenario::UpstreamCommitHistoricallyIntegratedOnTarget => {
            run_git(&repo_dir, &["checkout", "-b", "A", &a])?;
            append_and_commit(&repo_dir, "story.txt", "B\n", "B")?;
            let b = git_rev_parse(&repo_dir, "HEAD")?;
            append_and_commit(&repo_dir, "story.txt", "C\n", "C")?;

            run_git(&repo_dir, &["checkout", "-b", "target-main", &b])?;
            append_and_commit(&repo_dir, "story.txt", "X\n", "X")?;
            let x = git_rev_parse(&repo_dir, "HEAD")?;

            run_git(&repo_dir, &["checkout", "-b", "upstream-a", &x])?;
            append_and_commit(&repo_dir, "story.txt", "D\n", "D")?;
        }
    }

    let target_tip = git_rev_parse(&repo_dir, "target-main")?;
    let upstream_tip = git_rev_parse(&repo_dir, "upstream-a")?;
    run_git(
        &repo_dir,
        &["update-ref", "refs/remotes/origin/main", &target_tip],
    )?;
    run_git(
        &repo_dir,
        &["update-ref", "refs/remotes/origin/A", &upstream_tip],
    )?;
    run_git(&repo_dir, &["checkout", "A"])?;

    Ok((tmp, gix::open(repo_dir)?))
}

fn build_smart_squash_example_repo() -> Result<(TempDir, gix::Repository)> {
    let tmp = TempDir::new()?;
    let repo_dir = tmp.path().to_path_buf();

    run_git(&repo_dir, &["init", "--initial-branch=main"])?;
    run_git(&repo_dir, &["config", "user.name", "GitButler Tests"])?;
    run_git(&repo_dir, &["config", "user.email", "tests@gitbutler.com"])?;
    run_git(&repo_dir, &["config", "commit.gpgsign", "false"])?;

    write_file(&repo_dir, "story.txt", "base\n")?;
    run_git(&repo_dir, &["add", "story.txt"])?;
    run_git(&repo_dir, &["commit", "-m", "base"])?;
    let merge_base = git_rev_parse(&repo_dir, "HEAD")?;

    run_git(&repo_dir, &["checkout", "-b", "A", &merge_base])?;
    append_and_commit(&repo_dir, "story.txt", "local parent\n", "local parent")?;
    append_and_commit(&repo_dir, "story.txt", "local tip\n", "local tip")?;

    run_git(&repo_dir, &["checkout", "-b", "upstream-a", &merge_base])?;
    append_and_commit(&repo_dir, "story.txt", "remote parent\n", "remote parent")?;
    append_and_commit(&repo_dir, "story.txt", "remote tip\n", "remote tip")?;

    let main_tip = git_rev_parse(&repo_dir, "main")?;
    let upstream_tip = git_rev_parse(&repo_dir, "upstream-a")?;
    run_git(
        &repo_dir,
        &["update-ref", "refs/remotes/origin/main", &main_tip],
    )?;
    run_git(
        &repo_dir,
        &["update-ref", "refs/remotes/origin/A", &upstream_tip],
    )?;
    run_git(&repo_dir, &["checkout", "A"])?;

    Ok((tmp, gix::open(repo_dir)?))
}

fn append_and_commit(
    repo_dir: &std::path::Path,
    path: &str,
    content: &str,
    message: &str,
) -> Result<()> {
    let file_path = repo_dir.join(path);
    let mut current = std::fs::read_to_string(&file_path).unwrap_or_default();
    current.push_str(content);
    std::fs::write(file_path, current)?;
    run_git(repo_dir, &["add", path])?;
    run_git(repo_dir, &["commit", "-m", message])?;
    Ok(())
}

fn rewrite_commits_with_change_ids(
    repo: &gix::Repository,
    ref_name: &str,
    revs_parent_to_child: &[&str],
    change_ids: &[Option<ChangeId>],
) -> Result<Vec<gix::ObjectId>> {
    assert_eq!(
        revs_parent_to_child.len(),
        change_ids.len(),
        "test helper must get one Change-Id option per commit"
    );
    let mut rewritten_commit_ids = Vec::with_capacity(revs_parent_to_child.len());
    let old_commit_ids = revs_parent_to_child
        .iter()
        .map(|rev| repo.rev_parse_single(*rev).map(|id| id.detach()))
        .collect::<std::result::Result<Vec<_>, _>>()?;

    for (old_commit_id, change_id) in old_commit_ids.iter().zip(change_ids) {
        let mut commit = repo.find_commit(*old_commit_id)?.decode()?.to_owned()?;
        commit.tree = repo.find_commit(*old_commit_id)?.tree_id()?.detach();
        if let Some(parent_id) = rewritten_commit_ids.last().copied() {
            commit.parents = vec![parent_id].into();
        }

        if let Some(change_id) = change_id {
            Headers::from_change_id(change_id.clone()).set_in_commit(&mut commit);
        } else {
            Headers::remove_in_commit(&mut commit);
        }

        rewritten_commit_ids.push(repo.write_object(commit)?.detach());
    }

    let tip = *rewritten_commit_ids
        .last()
        .expect("test helper should be called with at least one commit");
    repo.reference(
        ref_name,
        tip,
        gix::refs::transaction::PreviousValue::Any,
        "rewrite commits with Change-Ids".as_bytes().as_bstr(),
    )?;

    Ok(rewritten_commit_ids)
}

fn write_file(repo_dir: &std::path::Path, path: &str, content: &str) -> Result<()> {
    std::fs::write(repo_dir.join(path), content)?;
    Ok(())
}

fn git_rev_parse(repo_dir: &std::path::Path, rev: &str) -> Result<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .arg("rev-parse")
        .arg(rev)
        .output()?;
    assert_eq!(
        output.status.code(),
        Some(0),
        "git rev-parse failed for {rev}"
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn run_git(repo_dir: &std::path::Path, args: &[&str]) -> Result<()> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(args)
        .output()?;
    assert_eq!(
        output.status.code(),
        Some(0),
        "git command failed: git {args:?}",
    );
    Ok(())
}

fn r(name: &str) -> &gix::refs::FullNameRef {
    name.try_into().expect("statically known valid ref-name")
}
