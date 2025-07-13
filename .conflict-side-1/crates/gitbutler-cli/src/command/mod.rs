pub mod prepare;
pub mod project;
pub mod vbranch;

fn debug_print(this: impl std::fmt::Debug) -> anyhow::Result<()> {
    println!("{:#?}", this);
    Ok(())
}

pub mod workspace {
    use crate::args::UpdateMode;
    use but_settings::AppSettings;
    use gitbutler_branch_actions::upstream_integration;
    use gitbutler_command_context::CommandContext;
    use gitbutler_project::Project;

    pub fn update(project: Project, mode: UpdateMode) -> anyhow::Result<()> {
        let approach = match mode {
            UpdateMode::Rebase => upstream_integration::ResolutionApproach::Rebase,
            UpdateMode::Merge => upstream_integration::ResolutionApproach::Merge,
            UpdateMode::Unapply => upstream_integration::ResolutionApproach::Unapply,
            UpdateMode::Delete => upstream_integration::ResolutionApproach::Delete,
        };
        let ctx = CommandContext::open(&project, AppSettings::default())?;
        let resolutions: Vec<_> = gitbutler_branch_actions::list_virtual_branches(&ctx)?
            .branches
            .into_iter()
            .map(|b| upstream_integration::Resolution {
                branch_id: b.id,
                approach,
                delete_integrated_branches: false,
                force_integrated_branches: vec![],
            })
            .collect();
        gitbutler_branch_actions::integrate_upstream(&ctx, &resolutions, None)?;

        Ok(())
    }
}
