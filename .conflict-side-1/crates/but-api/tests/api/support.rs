use but_core::ref_metadata::ProjectMeta;
use but_testsupport::gix_testtools::tempfile::TempDir;

pub fn writable_scenario(name: &str) -> (gix::Repository, TempDir) {
    but_testsupport::writable_scenario(name)
}

pub fn persist_default_target(repo: &gix::Repository) -> anyhow::Result<gix::ObjectId> {
    let target_commit_id = repo.rev_parse_single("refs/heads/main")?.detach();
    ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        target_commit_id: Some(target_commit_id),
        push_remote: Some("origin".into()),
    }
    .persist_to_local_config(repo)?;
    Ok(target_commit_id)
}

pub fn repository_graph(repo: &gix::Repository) -> anyhow::Result<String> {
    Ok(but_testsupport::visualize_commit_graph_all(repo)?)
}

pub fn workspace_graph(ctx: &but_ctx::Context) -> anyhow::Result<String> {
    let (_guard, _repo, ws, _db) = ctx.workspace_and_db()?;
    Ok(but_testsupport::graph_workspace(&ws).to_string())
}

pub fn fresh_head_info(ctx: &but_ctx::Context) -> anyhow::Result<but_workspace::RefInfo> {
    let project_meta = ctx.project_meta()?;
    let meta = ctx.meta()?;
    let repo = ctx.repo.get()?;
    but_workspace::head_info(
        &repo,
        &meta,
        but_workspace::ref_info::Options {
            project_meta,
            traversal: but_graph::init::Options::limited(),
            expensive_commit_info: true,
            ..Default::default()
        },
    )
    .map(but_workspace::RefInfo::pruned_to_entrypoint)
}
