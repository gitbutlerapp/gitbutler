use anyhow::Result;
use but_testsupport::visualize_commit_graph_all;
use but_workspace::branch::integrate_branch_upstream::{
    InteractiveIntegrationStep, get_initial_integration_steps_for_branch,
};

use crate::utils::{read_only_in_memory_scenario, read_only_in_memory_scenario_named};

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

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
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

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 1a265a4 (HEAD -> A) local change in A
    | * 89cc2d3 (origin/A) change in A
    |/  
    * d79bba9 new file in A
    * c166d42 (origin/main, origin/HEAD, main) init-integration
    ");

    let steps = get_initial_integration_steps_for_branch(r("refs/heads/A"), &repo)?;

    let local_tip = repo.rev_parse_single("A")?.detach();
    let upstream_tip = repo.rev_parse_single("origin/A")?.detach();
    let step_ids = pick_step_ids(&steps);

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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 0b1ed50 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * e9c9d74 (A) A2
    * 550b6ac A1
    | * ad92cce (origin/A) A2
    | * e1f216e A1
    |/  
    * fafd9d0 (origin/main, main) init
    ");

    let steps = get_initial_integration_steps_for_branch(r("refs/heads/A"), &repo)?;

    let local_only = repo.rev_parse_single("A~1")?.detach();
    let remote_only = repo.rev_parse_single("origin/A~1")?.detach();
    let local_and_remote = repo.rev_parse_single("A")?.detach();
    let step_ids = pick_step_ids(&steps);

    assert_eq!(
        step_ids,
        vec![remote_only, local_only, local_and_remote],
        "expected application order to build from the merge-base up to the rewritten local tip"
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

fn r(name: &str) -> &gix::refs::FullNameRef {
    name.try_into().expect("statically known valid ref-name")
}
