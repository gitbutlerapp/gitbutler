pub mod prepare;
pub mod project;
pub mod vbranch;

fn debug_print(this: impl std::fmt::Debug) -> anyhow::Result<()> {
    println!("{:#?}", this);
    Ok(())
}

pub mod ownership {
    use but_settings::AppSettings;
    use gitbutler_command_context::CommandContext;
    use gitbutler_diff::Hunk;
    use gitbutler_project::Project;
    use gitbutler_stack::{BranchOwnershipClaims, OwnershipClaim};
    use std::path::PathBuf;

    pub fn unapply(
        project: Project,
        file_path: PathBuf,
        from_line: u32,
        to_line: u32,
    ) -> anyhow::Result<()> {
        let claims = BranchOwnershipClaims {
            claims: vec![OwnershipClaim {
                file_path,
                hunks: vec![Hunk {
                    hash: None,
                    start: from_line,
                    end: to_line,
                    hunk_header: None,
                }],
            }],
        };

        let ctx = CommandContext::open(&project, AppSettings::default())?;
        gitbutler_branch_actions::unapply_ownership(&ctx, &claims)
    }
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
            })
            .collect();
        gitbutler_branch_actions::integrate_upstream(&ctx, &resolutions, None)?;

        Ok(())
    }
}
