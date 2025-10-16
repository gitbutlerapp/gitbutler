use std::str::FromStr;

use but_workspace::StackId;
use gitbutler_command_context::CommandContext;
use gix::ObjectId;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewordOutcome {
    pub stack_id: StackId,
    pub branch_name: String,
    #[serde(with = "gitbutler_serde::object_id")]
    pub commit_id: ObjectId,
    pub new_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameBranchOutcome {
    pub stack_id: StackId,
    pub old_branch_name: String,
    pub new_branch_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum Kind {
    Reword(Option<RewordOutcome>),
    RenameBranch(RenameBranchOutcome),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum KindCompat {
    String(String),
    KindRenameBranchObj {
        #[serde(rename = "type")]
        kind_type: String,
        subject: RenameBranchOutcome,
    },
    KindRewordObj {
        #[serde(rename = "type")]
        kind_type: String,
        #[serde(default)]
        subject: Option<RewordOutcome>,
    },
}

impl<'de> Deserialize<'de> for Kind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match KindCompat::deserialize(deserializer)? {
            KindCompat::String(s) if s == "Reword" => Ok(Kind::Reword(None)),
            KindCompat::KindRewordObj { kind_type, subject }
                if kind_type == "reword" || kind_type == "Reword" =>
            {
                Ok(Kind::Reword(subject))
            }
            KindCompat::KindRenameBranchObj { kind_type, subject }
                if kind_type == "renameBranch" || kind_type == "RenameBranch" =>
            {
                Ok(Kind::RenameBranch(subject))
            }

            _ => Err(serde::de::Error::custom("Unknown Kind variant")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum Status {
    Completed,
    Failed(String),
    Interupted(Uuid),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum Trigger {
    Manual,
    Snapshot(Uuid),
    #[default]
    Unknown,
}

/// Represents a workflow that was executed by GitButler.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Workflow {
    /// Unique identifier for the workflow.
    id: Uuid,
    /// The time when the workflow was captured.
    created_at: chrono::NaiveDateTime,
    /// The type of the workflow performed.
    kind: Kind,
    /// The trigger that initiated the workflow.
    triggered_by: Trigger,
    /// The status of the workflow.
    status: Status,
    /// Input commits
    #[serde(with = "gitbutler_serde::object_id_vec")]
    input_commits: Vec<ObjectId>,
    /// Output commits
    #[serde(with = "gitbutler_serde::object_id_vec")]
    output_commits: Vec<ObjectId>,
    /// Optional summary of the workflow
    summary: Option<String>,
}

impl TryFrom<but_db::Workflow> for Workflow {
    type Error = anyhow::Error;

    fn try_from(value: but_db::Workflow) -> Result<Self, Self::Error> {
        let kind = serde_json::from_str(&value.kind)?;
        let triggered_by = serde_json::from_str(&value.triggered_by)?;
        let status = serde_json::from_str(&value.status)?;
        let input_commits: Vec<ObjectId> =
            serde_json::from_str::<Vec<String>>(&value.input_commits)?
                .iter()
                .map(|c| ObjectId::from_str(c))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| anyhow::anyhow!("Failed to parse input commits: {}", e))?;
        let output_commits: Vec<ObjectId> =
            serde_json::from_str::<Vec<String>>(&value.output_commits)?
                .iter()
                .map(|c| ObjectId::from_str(c))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| anyhow::anyhow!("Failed to parse output commits: {}", e))?;
        let summary = value.summary.as_deref().map(|s| s.to_string());
        Ok(Self {
            id: Uuid::parse_str(&value.id)?,
            created_at: value.created_at,
            kind,
            triggered_by,
            status,
            input_commits,
            output_commits,
            summary,
        })
    }
}

impl TryFrom<Workflow> for but_db::Workflow {
    type Error = anyhow::Error;

    fn try_from(value: Workflow) -> Result<Self, Self::Error> {
        let kind = serde_json::to_string(&value.kind)?;
        let triggered_by = serde_json::to_string(&value.triggered_by)?;
        let status = serde_json::to_string(&value.status)?;
        let input_commits = serde_json::to_string(
            &value
                .input_commits
                .iter()
                .map(|c| c.to_string())
                .collect_vec(),
        )?;
        let output_commits = serde_json::to_string(
            &value
                .output_commits
                .iter()
                .map(|c| c.to_string())
                .collect_vec(),
        )?;
        let summary = value.summary.as_deref().map(|s| s.to_string());
        Ok(Self {
            id: value.id.to_string(),
            created_at: value.created_at,
            kind,
            triggered_by,
            status,
            input_commits,
            output_commits,
            summary,
        })
    }
}

impl Workflow {
    pub fn new(
        kind: Kind,
        triggered_by: Trigger,
        status: Status,
        input_commits: Vec<ObjectId>,
        output_commits: Vec<ObjectId>,
        summary: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            created_at: chrono::Local::now().naive_local(),
            kind,
            triggered_by,
            status,
            input_commits,
            output_commits,
            summary,
        }
    }

    pub(crate) fn persist(self, ctx: &mut CommandContext) -> anyhow::Result<()> {
        ctx.db()?
            .workflows()
            .insert(self.try_into()?)
            .map_err(|e| anyhow::anyhow!("Failed to persist workflow: {}", e))?;
        Ok(())
    }
}

pub fn list_workflows(
    ctx: &mut CommandContext,
    offset: i64,
    limit: i64,
) -> anyhow::Result<WorkflowList> {
    let (total, workflows) = ctx
        .db()?
        .workflows()
        .list(offset, limit)
        .map_err(|e| anyhow::anyhow!("Failed to list workflows: {}", e))?;

    let workflows = workflows
        .into_iter()
        .map(|w| w.try_into())
        .collect::<Result<Vec<Workflow>, _>>()?;

    Ok(WorkflowList { total, workflows })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowList {
    total: i64,
    workflows: Vec<Workflow>,
}
