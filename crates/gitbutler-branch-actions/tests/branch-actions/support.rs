use anyhow::Result;
use but_core::ref_metadata::StackId;
use but_ctx::{Context, RepoOpenMode};
use but_settings::AppSettings;
use but_testsupport::gix_testtools::{Creation, scripted_fixture_writable_with_args};
use but_workspace::branch::Stack;
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

/// The applied stacks of `ctx` that are recorded in workspace metadata, as the workspace
/// projects them.
///
/// An ad-hoc workspace (a plain branch checkout) is projected as a single stack under the
/// synthetic [`StackId::single_branch_id()`]; it isn't a metadata stack, so it is left out.
pub fn stack_details(ctx: &Context) -> Vec<(StackId, Stack)> {
    let repo = ctx.clone_repo_for_merging_non_persisting().unwrap();
    let meta = ctx.legacy_meta().unwrap();
    let mut db = ctx.db.get_cache_mut().unwrap();
    but_workspace::head_info(
        &repo,
        &meta,
        &mut db,
        but_workspace::ref_info::Options {
            project_meta: ctx.project_meta().unwrap(),
            traversal: but_graph::init::Options {
                worktrees: ctx.settings.feature_flags.worktree_manipulation,
                ..but_graph::init::Options::limited()
            },
            expensive_commit_info: true,
            ..Default::default()
        },
    )
    .unwrap()
    .pruned_to_entrypoint()
    .stacks
    .into_iter()
    .filter_map(|stack| Some((stack.id?, stack)))
    .filter(|(id, _)| *id != StackId::single_branch_id())
    .collect()
}

/// The short name of the stack's top-most branch.
pub fn stack_name(stack: &Stack) -> String {
    stack
        .name()
        .expect("stacks in a workspace are named")
        .shorten()
        .to_string()
}
