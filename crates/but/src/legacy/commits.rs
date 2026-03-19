use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_workspace::{
    legacy::{StacksFilter, ui::StackEntry},
    ui::StackDetails,
};

pub fn stacks(ctx: &Context) -> anyhow::Result<Vec<StackEntry>> {
    let repo = ctx.clone_repo_for_merging_non_persisting()?;
    let meta = ctx.meta()?;
    let mut cache = ctx.cache.get_cache_mut()?;
    but_workspace::legacy::stacks_v3(&repo, &meta, StacksFilter::default(), None, &mut cache)
}

pub fn stack_details(ctx: &Context, stack_id: StackId) -> anyhow::Result<StackDetails> {
    let repo = ctx.clone_repo_for_merging_non_persisting()?;
    let meta = ctx.meta()?;
    let mut cache = ctx.cache.get_cache_mut()?;
    but_workspace::legacy::stack_details_v3(Some(stack_id), &repo, &meta, &mut cache)
}
