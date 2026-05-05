use std::vec;

use anyhow::{Result, bail};
use but_core::Commit;
use but_rebase::graph_rebase::Editor;
use but_testsupport::{visualize_commit_graph_all, visualize_tree};
use but_workspace::branch::integrate_branch_upstream::{
    InitialBranchIntegration, IntegrationDivergenceCommit, InteractiveIntegration,
    InteractiveIntegrationStep, get_initial_integration_steps_for_branch,
    integrate_branch_with_steps,
};
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
            InteractiveIntegrationStep::Skip { commit_id } => {
                out.push_str("skip ");
                out.push_str(&label_for(*commit_id, labels));
            }
            InteractiveIntegrationStep::Pick { commit_id } => {
                out.push_str("pick ");
                out.push_str(&label_for(*commit_id, labels));
            }
            InteractiveIntegrationStep::PickUpstream { commit_id } => {
                out.push_str("pick-upstream ");
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
        format!(
            "{prefix}{}{} {}",
            label_for(commit.id, labels),
            refs,
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
    for commit in &initial.divergence.matched {
        out.push(render_commit("* ", commit, labels));
    }
    out.push(render_commit("* ", &initial.divergence.merge_base, labels));
    out.join("\n")
}

fn label_for(id: gix::ObjectId, labels: &[(gix::ObjectId, &str)]) -> String {
    labels
        .iter()
        .find_map(|(candidate, label)| (*candidate == id).then_some(*label))
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| id.to_string())
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

    let err = get_initial_integration_steps_for_branch(r("refs/heads/A"), &repo)
        .expect_err("branch without tracking must fail");

    assert!(
        err.to_string().contains("has no tracking branch"),
        "unexpected error: {err:#}"
    );

    Ok(())
}

#[test]
fn partitions_diverged_branch_into_application_order() -> Result<()> {
    let repo = read_only_in_memory_scenario_named("with-remotes-no-workspace", "remote-diverged")?;

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
    let integration = get_initial_integration_steps_for_branch(r("refs/heads/A"), &repo)?;

    insta::assert_snapshot!(
        labeled_integration_snapshot(
            &integration,
            &[
                (merge_base, "merge-base"),
                (local_tip, "local-tip"),
                (upstream_tip, "upstream-tip"),
            ]
        ),
        @"
    merge-base merge-base
    pick-upstream upstream-tip
    pick local-tip
    "
    );

    let step_ids = pick_step_ids(&integration.steps);

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

    let initial = get_initial_integration_steps_for_branch(r("refs/heads/A"), &repo)?;

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
    pick-upstream upstream-tip
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
fn matches_rewritten_commit_by_change_id_and_keeps_order() -> Result<()> {
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
    let local_and_remote = repo.rev_parse_single("A")?.detach();
    let merge_base = repo.rev_parse_single("A~2")?.detach();
    let integration = get_initial_integration_steps_for_branch(r("refs/heads/A"), &repo)?;

    insta::assert_snapshot!(
        labeled_integration_snapshot(
            &integration,
            &[
                (merge_base, "merge-base"),
                (local_only, "local-only"),
                (remote_only, "remote-only"),
                (local_and_remote, "local-and-remote"),
            ]
        ),
        @"
    merge-base merge-base
    pick-upstream remote-only
    pick local-only
    pick local-and-remote
    "
    );

    let step_ids = pick_step_ids(&integration.steps);

    assert_eq!(
        step_ids,
        vec![remote_only, local_only, local_and_remote],
        "expected application order to build from the merge-base up to the rewritten local tip"
    );
    Ok(())
}

#[test]
fn integrate_branch_with_steps_empty_errors_early() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
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
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let merge_base = repo.rev_parse_single("main")?.detach();
    let integration = InteractiveIntegration {
        merge_base,
        steps: vec![],
    };

    let err = integrate_branch_with_steps(editor, r("refs/heads/B"), integration)
        .expect_err("expected early validation error for empty integration steps");
    assert!(
        err.to_string()
            .contains("Integration steps cannot be empty"),
        "unexpected error: {err:#}"
    );

    Ok(())
}

#[test]
fn integrate_upstream_commits_into_local() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
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
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let local_commit_2 = repo.rev_parse_single("A")?.detach();
    let local_commit_1 = repo.rev_parse_single("A~1")?.detach();
    let remote_commit_2 = repo.rev_parse_single("origin/A")?.detach();
    let remote_commit_1 = repo.rev_parse_single("origin/A~1")?.detach();
    let local_and_remote = repo.rev_parse_single("A~2")?.detach();
    let steps = vec![
        InteractiveIntegrationStep::PickUpstream {
            commit_id: remote_commit_1,
        },
        InteractiveIntegrationStep::PickUpstream {
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
        steps,
    };

    let rebase = integrate_branch_with_steps(editor, r("refs/heads/A"), integration)?;
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

    Ok(())
}

#[test]
fn integrate_upstream_commits_into_local_with_remote_on_top() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
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
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

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
        InteractiveIntegrationStep::PickUpstream {
            commit_id: remote_commit_1,
        },
        InteractiveIntegrationStep::PickUpstream {
            commit_id: remote_commit_2,
        },
    ];

    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        steps,
    };

    let rebase = integrate_branch_with_steps(editor, r("refs/heads/A"), integration)?;
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
    let (_tmp, graph, repo, mut meta, _description) =
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
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let local_commit_2 = repo.rev_parse_single("A")?.detach();
    let local_commit_1 = repo.rev_parse_single("A~1")?.detach();
    let remote_commit_2 = repo.rev_parse_single("origin/A")?.detach();
    let remote_commit_1 = repo.rev_parse_single("origin/A~1")?.detach();
    let local_and_remote = repo.rev_parse_single("A~2")?.detach();
    let steps = vec![
        InteractiveIntegrationStep::PickUpstream {
            commit_id: remote_commit_1,
        },
        InteractiveIntegrationStep::Pick {
            commit_id: local_commit_1,
        },
        InteractiveIntegrationStep::PickUpstream {
            commit_id: remote_commit_2,
        },
        InteractiveIntegrationStep::Pick {
            commit_id: local_commit_2,
        },
    ];

    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        steps,
    };

    let rebase = integrate_branch_with_steps(editor, r("refs/heads/A"), integration)?;
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
    let (_tmp, graph, repo, mut meta, _description) =
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
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let local_commit_2 = repo.rev_parse_single("A")?.detach();
    let local_commit_1 = repo.rev_parse_single("A~1")?.detach();
    let remote_commit_2 = repo.rev_parse_single("origin/A")?.detach();
    let remote_commit_1 = repo.rev_parse_single("origin/A~1")?.detach();
    let local_and_remote = repo.rev_parse_single("A~2")?.detach();
    let steps = vec![
        InteractiveIntegrationStep::Pick {
            commit_id: local_commit_1,
        },
        InteractiveIntegrationStep::Skip {
            commit_id: local_commit_2,
        },
        InteractiveIntegrationStep::Skip {
            commit_id: remote_commit_1,
        },
        InteractiveIntegrationStep::PickUpstream {
            commit_id: remote_commit_2,
        },
    ];

    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        steps,
    };

    let rebase = integrate_branch_with_steps(editor, r("refs/heads/A"), integration)?;
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
    let (_tmp, graph, repo, mut meta, _description) =
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
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let local_commit_2 = repo.rev_parse_single("A")?.detach();
    let local_commit_1 = repo.rev_parse_single("A~1")?.detach();
    let remote_commit_2 = repo.rev_parse_single("origin/A")?.detach();
    let remote_commit_1 = repo.rev_parse_single("origin/A~1")?.detach();
    let local_and_remote = repo.rev_parse_single("A~2")?.detach();
    let steps = vec![
        InteractiveIntegrationStep::Pick {
            commit_id: local_commit_1,
        },
        InteractiveIntegrationStep::Skip {
            commit_id: local_commit_2,
        },
        InteractiveIntegrationStep::Skip {
            commit_id: remote_commit_1,
        },
        InteractiveIntegrationStep::PickUpstream {
            commit_id: remote_commit_2,
        },
    ];

    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        steps,
    };

    let rebase = integrate_branch_with_steps(editor, r("refs/heads/A"), integration)?;
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
    let (_tmp, graph, repo, mut meta, _description) =
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
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let local_commit_2 = repo.rev_parse_single("A")?.detach();
    let local_commit_1 = repo.rev_parse_single("A~1")?.detach();
    let remote_commit_2 = repo.rev_parse_single("origin/A")?.detach();
    let remote_commit_1 = repo.rev_parse_single("origin/A~1")?.detach();
    let local_and_remote = repo.rev_parse_single("A~2")?.detach();
    let steps = vec![
        InteractiveIntegrationStep::Skip {
            commit_id: local_commit_1,
        },
        InteractiveIntegrationStep::Skip {
            commit_id: local_commit_2,
        },
        InteractiveIntegrationStep::PickUpstream {
            commit_id: remote_commit_1,
        },
        InteractiveIntegrationStep::PickUpstream {
            commit_id: remote_commit_2,
        },
    ];

    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        steps,
    };

    let rebase = integrate_branch_with_steps(editor, r("refs/heads/A"), integration)?;
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
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "remote-diverged-with-workspace",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            },
        )?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let local_commit_2 = repo.rev_parse_single("A")?.detach();
    let local_commit_1 = repo.rev_parse_single("A~1")?.detach();
    let remote_commit_2 = repo.rev_parse_single("origin/A")?.detach();
    let remote_commit_1 = repo.rev_parse_single("origin/A~1")?.detach();
    let local_and_remote = repo.rev_parse_single("A~2")?.detach();
    let steps = vec![
        InteractiveIntegrationStep::PickUpstream {
            commit_id: remote_commit_1,
        },
        InteractiveIntegrationStep::PickUpstream {
            commit_id: remote_commit_2,
        },
        InteractiveIntegrationStep::Squash {
            commits: vec![local_commit_1, local_commit_2],
            message: Some("squashed local commits".to_string()),
        },
    ];

    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        steps,
    };

    let rebase = integrate_branch_with_steps(editor, r("refs/heads/A"), integration)?;
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
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "remote-diverged-with-workspace",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            },
        )?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

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
        steps,
    };

    let rebase = integrate_branch_with_steps(editor, r("refs/heads/A"), integration)?;
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
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "remote-diverged-with-workspace",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            },
        )?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

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
        steps,
    };

    let rebase = integrate_branch_with_steps(editor, r("refs/heads/A"), integration)?;
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
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "remote-diverged-with-workspace-conflicting-squash",
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
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let local_commit_1 = repo.rev_parse_single("A")?.detach();
    let remote_commit_1 = repo.rev_parse_single("origin/A")?.detach();
    let local_and_remote = repo.rev_parse_single("main")?.detach();
    let steps = vec![InteractiveIntegrationStep::Squash {
        commits: vec![remote_commit_1, local_commit_1],
        message: Some("squashed conflicting commits".to_string()),
    }];

    let integration = InteractiveIntegration {
        merge_base: local_and_remote,
        steps,
    };

    let rebase = integrate_branch_with_steps(editor, r("refs/heads/A"), integration)?;
    rebase.materialize()?;

    insta::assert_snapshot!(normalized_graph_snapshot(&repo)?, @"
    * f03fc2c (origin/A, new-origin) remote change in A 1
    | * 5b134d5 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * 13b9b63 (A) [conflict] squashed conflicting commits
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
    insta::assert_snapshot!(visualize_tree(branch_tip.tree_id()?), @r#"
    accf9f2
    ├── .auto-resolution:cd74779 
    │   └── shared.txt:100644:9c998f7 "remote\n"
    ├── .conflict-base-0:48e531d 
    │   └── shared.txt:100644:df967b9 "base\n"
    ├── .conflict-files:100644:d0a3da4 "ancestorEntries = [\"shared.txt\"]\nourEntries = [\"shared.txt\"]\ntheirEntries = [\"shared.txt\"]\n"
    ├── .conflict-side-0:cd74779 
    │   └── shared.txt:100644:9c998f7 "remote\n"
    ├── .conflict-side-1:276d2b4 
    │   └── shared.txt:100644:4083037 "local\n"
    └── shared.txt:100644:9c998f7 "remote\n"
    "#);

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
    let integration = get_initial_integration_steps_for_branch(r("refs/heads/A"), &repo)?;

    insta::assert_snapshot!(
        labeled_integration_snapshot(
            &integration,
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
    pick-upstream remote-commit-1
    pick-upstream remote-commit-2
    pick local-commit-1
    pick local-commit-2
    "
    );

    assert_eq!(
        pick_step_ids(&integration.steps),
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
            | InteractiveIntegrationStep::Skip { commit_id, .. }
            | InteractiveIntegrationStep::PickUpstream { commit_id, .. } => *commit_id,
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

fn r(name: &str) -> &gix::refs::FullNameRef {
    name.try_into().expect("statically known valid ref-name")
}
