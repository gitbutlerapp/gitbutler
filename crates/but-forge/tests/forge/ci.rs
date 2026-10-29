use but_forge::{CiCheck, CiConclusion, CiStatus};

fn job(status: &str, web_url: Option<&str>) -> but_gitlab::GitLabPipelineJob {
    but_gitlab::GitLabPipelineJob {
        id: 42,
        name: "job".into(),
        status: status.into(),
        allow_failure: false,
        started_at: Some("2026-05-01T12:00:00Z".into()),
        finished_at: Some("2026-05-01T12:05:00Z".into()),
        web_url: web_url.map(str::to_owned),
        pipeline: but_gitlab::GitLabPipelineRef {
            id: 7,
            web_url: None,
            status: None,
        },
    }
}

#[test]
fn maps_manual_jobs_to_action_required_complete_status() {
    let check = CiCheck::from(job("manual", Some("https://example.com/job")));

    assert!(matches!(
        check.status,
        CiStatus::Complete {
            conclusion: CiConclusion::ActionRequired,
            ..
        }
    ));
}

#[test]
fn maps_allowed_failure_jobs_to_neutral_complete_status() {
    let mut job = job("failed", Some("https://example.com/job"));
    job.allow_failure = true;

    let check = CiCheck::from(job);

    assert!(matches!(
        check.status,
        CiStatus::Complete {
            conclusion: CiConclusion::Neutral,
            ..
        }
    ));
}

#[test]
fn maps_optional_manual_jobs_to_neutral_complete_status() {
    let mut job = job("manual", Some("https://example.com/job"));
    job.allow_failure = true;

    let check = CiCheck::from(job);

    assert!(matches!(
        check.status,
        CiStatus::Complete {
            conclusion: CiConclusion::Neutral,
            ..
        }
    ));
}

#[test]
fn maps_canceling_jobs_to_in_progress_status() {
    let check = CiCheck::from(job("canceling", Some("https://example.com/job")));

    assert!(matches!(check.status, CiStatus::InProgress));
}

#[test]
fn maps_waiting_for_callback_jobs_to_queued_status() {
    let check = CiCheck::from(job("waiting_for_callback", Some("https://example.com/job")));

    assert!(matches!(check.status, CiStatus::Queued));
}

fn bb_status(state: &str) -> but_bitbucket::BitbucketBuildStatus {
    but_bitbucket::BitbucketBuildStatus {
        key: "PIPELINE-1".into(),
        name: "Build".into(),
        description: Some("desc".into()),
        state: state.into(),
        url: Some("https://bitbucket.org/ws/repo/pipelines/1".into()),
        commit_hash: "deadbeef".into(),
        created_on: Some("2026-05-01T12:00:00Z".into()),
        updated_on: Some("2026-05-01T12:05:00Z".into()),
    }
}

#[test]
fn maps_bitbucket_build_states() {
    assert!(matches!(
        CiCheck::from(bb_status("SUCCESSFUL")).status,
        CiStatus::Complete {
            conclusion: CiConclusion::Success,
            ..
        }
    ));
    assert!(matches!(
        CiCheck::from(bb_status("FAILED")).status,
        CiStatus::Complete {
            conclusion: CiConclusion::Failure,
            ..
        }
    ));
    assert!(matches!(
        CiCheck::from(bb_status("STOPPED")).status,
        CiStatus::Complete {
            conclusion: CiConclusion::Cancelled,
            ..
        }
    ));
    assert!(matches!(
        CiCheck::from(bb_status("INPROGRESS")).status,
        CiStatus::InProgress
    ));
    assert!(matches!(
        CiCheck::from(bb_status("WEIRD")).status,
        CiStatus::Unknown
    ));
}

#[test]
fn bitbucket_build_status_id_is_stable_and_nonzero() {
    let a = CiCheck::from(bb_status("SUCCESSFUL"));
    let b = CiCheck::from(bb_status("SUCCESSFUL"));
    assert_eq!(a.id, b.id);
    assert_ne!(a.id, 0);
    assert_eq!(a.head_sha, "deadbeef");
}

#[test]
fn bitbucket_build_status_id_differs_across_commits_and_keys() {
    let base = bb_status("SUCCESSFUL");

    // Same key on a different commit must not collide (the id is the
    // ci_checks primary key, and keys like "build" recur across refs/repos).
    let other_commit = but_bitbucket::BitbucketBuildStatus {
        commit_hash: "feedface".into(),
        ..bb_status("SUCCESSFUL")
    };
    assert_ne!(CiCheck::from(base).id, CiCheck::from(other_commit).id);

    // Different key on the same commit also differs.
    let other_key = but_bitbucket::BitbucketBuildStatus {
        key: "PIPELINE-2".into(),
        ..bb_status("SUCCESSFUL")
    };
    assert_ne!(
        CiCheck::from(bb_status("SUCCESSFUL")).id,
        CiCheck::from(other_key).id
    );
}
