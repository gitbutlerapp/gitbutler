use crate::ForgeName;

pub fn ci_checks_for_ref(
    preferred_forge_user: Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    storage: &but_forge_storage::Controller,
    reference: &str,
) -> anyhow::Result<Vec<CiCheck>> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.github().cloned());
            let gh = but_github::GitHubClient::from_storage(storage, preferred_account.as_ref())?;

            // Clone owned datta for thread
            let owner = owner.clone();
            let repo = repo.clone();
            let reference = reference.to_string();

            let checks = std::thread::spawn(move || {
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(gh.list_checks_for_ref(&owner, &repo, &reference))
            })
            .join()
            .map_err(|e| anyhow::anyhow!("Failed to join thread: {:?}", e))?;
            checks.map(|c| c.into_iter().map(CiCheck::from).collect())
        }
        _ => Err(anyhow::anyhow!(
            "Listing ci checks for forge {:?} is not implemented yet.",
            forge
        )),
    }
}

pub struct CiCheck {
    pub id: i64,
    pub name: String,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub status: CiStatus,
    pub head_sha: String,
    pub url: String,
    pub html_url: String,
    pub details_url: String,
    pub pull_requests: Vec<PullRequestMinimal>,
}

pub struct CiOutput {
    pub summary: String,
    pub text: String,
    pub title: String,
}

pub enum CiStatus {
    Complete {
        conclusion: CiConclusion,
        completed_at: chrono::DateTime<chrono::Utc>,
    },
    InProgress,
    Queued,
    Unknown,
}

pub enum CiConclusion {
    ActionRequired,
    Cancelled,
    Failure,
    Neutral,
    Skipped,
    Success,
    TimedOut,
    Unknown,
}

pub struct PullRequestMinimal {
    pub id: i64,
    pub number: i64,
    pub url: String,
    pub base_ref: String,
    pub base_repo_url: Option<String>,
    pub head_ref: String,
    pub head_repo_url: Option<String>,
}

impl From<octorust::types::PullRequestMinimal> for PullRequestMinimal {
    fn from(value: octorust::types::PullRequestMinimal) -> Self {
        PullRequestMinimal {
            id: value.id,
            number: value.number,
            url: value.url,
            base_ref: value.base.ref_,
            base_repo_url: value.base.repo.map(|repo| repo.url),
            head_ref: value.head.ref_,
            head_repo_url: value.head.repo.map(|repo| repo.url),
        }
    }
}

impl From<octorust::types::Conclusion> for CiConclusion {
    fn from(value: octorust::types::Conclusion) -> Self {
        match value {
            octorust::types::Conclusion::ActionRequired => CiConclusion::ActionRequired,
            octorust::types::Conclusion::Cancelled => CiConclusion::Cancelled,
            octorust::types::Conclusion::Failure => CiConclusion::Failure,
            octorust::types::Conclusion::Neutral => CiConclusion::Neutral,
            octorust::types::Conclusion::Skipped => CiConclusion::Skipped,
            octorust::types::Conclusion::Success => CiConclusion::Success,
            octorust::types::Conclusion::TimedOut => CiConclusion::TimedOut,
            octorust::types::Conclusion::Noop => CiConclusion::Unknown,
            octorust::types::Conclusion::FallthroughString => CiConclusion::Unknown,
        }
    }
}

impl From<octorust::types::CheckRun> for CiCheck {
    fn from(value: octorust::types::CheckRun) -> Self {
        let status = match value.status {
            octorust::types::JobStatus::Completed => {
                if let (Some(conclusion), Some(completed_at)) =
                    (value.conclusion, value.completed_at)
                {
                    CiStatus::Complete {
                        conclusion: conclusion.into(),
                        completed_at,
                    }
                } else {
                    CiStatus::Unknown
                }
            }
            octorust::types::JobStatus::InProgress => CiStatus::InProgress,
            octorust::types::JobStatus::Queued => CiStatus::Queued,
            octorust::types::JobStatus::Noop => CiStatus::Unknown,
            octorust::types::JobStatus::FallthroughString => CiStatus::Unknown,
        };
        CiCheck {
            id: value.id,
            name: value.name,
            started_at: value.started_at,
            status,
            head_sha: value.head_sha,
            url: value.url,
            html_url: value.html_url,
            details_url: value.details_url,
            pull_requests: value
                .pull_requests
                .into_iter()
                .map(|pr| pr.into())
                .collect(),
        }
    }
}
