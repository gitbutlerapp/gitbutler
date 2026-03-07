use crate::ProjectHandleOrLegacyProjectId;

impl TryFrom<ProjectHandleOrLegacyProjectId> for crate::Context {
    type Error = anyhow::Error;

    fn try_from(value: ProjectHandleOrLegacyProjectId) -> Result<Self, Self::Error> {
        match value {
            ProjectHandleOrLegacyProjectId::ProjectHandle(project_handle) => {
                crate::Context::try_from(project_handle)
            }
            #[cfg(feature = "legacy")]
            ProjectHandleOrLegacyProjectId::LegacyProjectId(project_id) => {
                crate::Context::try_from(project_id)
            }
        }
    }
}

impl TryFrom<ProjectHandleOrLegacyProjectId> for crate::ThreadSafeContext {
    type Error = anyhow::Error;

    fn try_from(value: ProjectHandleOrLegacyProjectId) -> Result<Self, Self::Error> {
        Ok(crate::Context::try_from(value)?.into_sync())
    }
}
