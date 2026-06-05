#![expect(
    deprecated,
    reason = "calls but_workspace::legacy::stacks_v3 and but_workspace::legacy::stack_details_v3"
)]

use anyhow::Result;
use but_core::ref_metadata::StackId;
use but_ctx::{Context, RepoOpenMode};
use but_settings::AppSettings;
use but_testsupport::gix_testtools::{Creation, scripted_fixture_writable_with_args};
use but_workspace::{legacy::StacksFilter, ui::StackDetails};
use tempfile::{TempDir, tempdir};

pub struct HookCase {
    pub ctx: Context,
    _app_data_dir: TempDir,
    _repo_dir: TempDir,
}

pub fn hook_case() -> Result<HookCase> {
    let repo_dir = scripted_fixture_writable_with_args(
        "scenario/repo-with-origin.sh",
        None::<String>,
        Creation::Execute,
    )
    .map_err(anyhow::Error::from_boxed)?;
    let local_repo_dir = repo_dir.path().join("local");
    let app_data_dir = tempdir()?;
    let project = gitbutler_project::add_at_app_data_dir(app_data_dir.path(), &local_repo_dir)?
        .unwrap_project();
    let ctx = Context::new_from_legacy_project_and_settings_with_repo_open_mode(
        &project,
        AppSettings::default(),
        RepoOpenMode::Isolated,
    )?
    .with_memory_app_cache();
    Ok(HookCase {
        ctx,
        _app_data_dir: app_data_dir,
        _repo_dir: repo_dir,
    })
}

pub fn stack_details(ctx: &Context) -> Vec<(StackId, StackDetails)> {
    let repo = ctx.clone_repo_for_merging_non_persisting().unwrap();
    let stacks = {
        let meta = ctx.legacy_meta().unwrap();
        but_workspace::legacy::stacks_v3(
            &repo,
            &meta,
            &ctx.project_meta().unwrap(),
            StacksFilter::default(),
            None,
        )
    }
    .unwrap();

    stacks
        .into_iter()
        .filter_map(|stack| {
            let stack_id = stack.id?;
            let details = {
                let meta = ctx.legacy_meta().unwrap();
                but_workspace::legacy::stack_details_v3(
                    stack_id.into(),
                    &repo,
                    &meta,
                    &ctx.project_meta().unwrap(),
                )
            }
            .unwrap();
            Some((stack_id, details))
        })
        .collect()
}
