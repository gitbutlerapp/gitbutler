use super::ForgeReview;

impl TryFrom<ForgeReview> for but_db::ForgeReview {
    type Error = anyhow::Error;
    fn try_from(value: ForgeReview) -> anyhow::Result<Self, Self::Error> {
        fn parse_datetime(datetime_str: &Option<String>) -> Option<chrono::NaiveDateTime> {
            datetime_str
                .as_ref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.naive_local())
        }
        let version = ForgeReview::struct_version();
        Ok(but_db::ForgeReview {
            html_url: value.html_url,
            number: value.number,
            title: value.title,
            body: value.body,
            author: match value.author {
                Some(ref author) => Some(serde_json::to_string(author)?),
                None => None,
            },
            labels: serde_json::to_string(&value.labels)?,
            draft: value.draft,
            source_branch: value.source_branch,
            target_branch: value.target_branch,
            sha: value.sha,
            created_at: parse_datetime(&value.created_at),
            modified_at: parse_datetime(&value.modified_at),
            merged_at: parse_datetime(&value.merged_at),
            closed_at: parse_datetime(&value.closed_at),
            repository_ssh_url: value.repository_ssh_url,
            repository_https_url: value.repository_https_url,
            repo_owner: value.repo_owner,
            reviewers: serde_json::to_string(&value.reviewers)?,
            unit_symbol: value.unit_symbol,
            last_sync_at: value.last_sync_at,
            struct_version: version,
        })
    }
}

impl TryFrom<but_db::ForgeReview> for ForgeReview {
    type Error = anyhow::Error;
    fn try_from(value: but_db::ForgeReview) -> anyhow::Result<Self, Self::Error> {
        fn to_iso_8601(datetime: &Option<chrono::NaiveDateTime>) -> Option<String> {
            datetime.map(|dt| dt.format("%+").to_string())
        }
        if value.struct_version != ForgeReview::struct_version() {
            return Err(anyhow::Error::msg(format!(
                "Incompatible ForgeReview struct version: expected {}, found {}",
                ForgeReview::struct_version(),
                value.struct_version
            )));
        }
        Ok(ForgeReview {
            html_url: value.html_url,
            number: value.number,
            title: value.title,
            body: value.body,
            author: match value.author {
                Some(ref author_str) => Some(serde_json::from_str(author_str)?),
                None => None,
            },
            labels: serde_json::from_str(&value.labels)?,
            draft: value.draft,
            source_branch: value.source_branch,
            target_branch: value.target_branch,
            sha: value.sha,
            created_at: to_iso_8601(&value.created_at),
            modified_at: to_iso_8601(&value.modified_at),
            merged_at: to_iso_8601(&value.merged_at),
            closed_at: to_iso_8601(&value.closed_at),
            repository_ssh_url: value.repository_ssh_url,
            repository_https_url: value.repository_https_url,
            repo_owner: value.repo_owner,
            reviewers: serde_json::from_str(&value.reviewers)?,
            unit_symbol: value.unit_symbol,
            last_sync_at: value.last_sync_at,
        })
    }
}
