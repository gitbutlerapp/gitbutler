use anyhow::Result;
use but_ctx::Context;

impl TryFrom<but_db::WorkspaceRule> for crate::WorkspaceRule {
    type Error = anyhow::Error;
    fn try_from(value: but_db::WorkspaceRule) -> Result<Self, Self::Error> {
        Ok(crate::WorkspaceRule {
            id: value.id,
            created_at: value.created_at,
            enabled: value.enabled,
            trigger: serde_json::from_str(&value.trigger)?,
            filters: serde_json::from_str(&value.filters)?,
            action: serde_json::from_str(&value.action)?,
        })
    }
}

impl TryFrom<crate::WorkspaceRule> for but_db::WorkspaceRule {
    type Error = anyhow::Error;
    fn try_from(value: crate::WorkspaceRule) -> Result<Self, Self::Error> {
        Ok(but_db::WorkspaceRule {
            id: value.id,
            created_at: value.created_at,
            enabled: value.enabled,
            trigger: serde_json::to_string(&value.trigger)?,
            filters: serde_json::to_string(&value.filters)?,
            action: serde_json::to_string(&value.action)?,
        })
    }
}

pub fn workspace_rules(ctx: &mut Context) -> Result<Vec<crate::WorkspaceRule>> {
    let rules = ctx
        .db
        .get_mut()?
        .workspace_rules()
        .list()?
        .into_iter()
        .map(|r| r.try_into())
        .collect::<Result<Vec<crate::WorkspaceRule>>>()?;
    Ok(rules)
}
