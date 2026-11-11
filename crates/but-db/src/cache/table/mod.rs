use crate::M;

pub const APP_MIGRATIONS: &[&[M<'static>]] = &[update::M, agent_skill_notice::M];
pub const PROJECT_MIGRATIONS: &[&[M<'static>]] = &[removed_change_ids::M];

pub(crate) mod agent_skill_notice;
pub(crate) mod removed_change_ids;
pub(crate) mod update;
