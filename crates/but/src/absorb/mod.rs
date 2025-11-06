use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;

use crate::{id::CliId, rub::parse_sources};

/// Amends changes into the appropriate commits where they belong.
///
/// The semantic for finding "the appropriate commit" is as follows
/// - Changes are amended into the topmost commit of the leftmost (first) lane (branch)
/// - If a change is assigned to a particular lane (branch), it will be amended into a commit there
///     - If there are no commits in this branch, a new commit is created
/// - If a change has a dependency to a particular commit, it will be amended into that particular commit
///
/// Optionally an identifier to an Uncommitted File or a Branch (stack) may be provided.
///
/// If an Uncommitted File id is provided, absorb will be peformed for just that file
/// If a Branch (stack) id is provided, absorb will be performed for all changes assigned to that stack
/// If no source is provided, absorb is performed for all uncommitted changes
pub(crate) fn handle(project: &Project, _json: bool, source: Option<&str>) -> anyhow::Result<()> {
    let ctx = &mut CommandContext::open(project, AppSettings::load_from_default_path_creating()?)?;
    let source: Option<CliId> = source
        .and_then(|s| parse_sources(ctx, s).ok())
        .and_then(|s| {
            s.into_iter().find(|s| {
                matches!(s, CliId::UncommittedFile { .. }) || matches!(s, CliId::Branch { .. })
            })
        });
    if let Some(source) = source {
        match source {
            CliId::UncommittedFile {
                path: _,
                assignment: _,
            } => {
                // Absorb this particular file
            }
            CliId::Branch { name: _ } => {
                // Absorb everything that is assigned to this lane
            }
            _ => {
                // Invalid source - error out
            }
        }
    } else {
        // Try to absorb everhting uncommitted
    }
    Ok(())
}
