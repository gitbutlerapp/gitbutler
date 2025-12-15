use std::io::{Write, stdout};

use gix::odb::store::RefreshMode;

pub enum Dot {
    Print,
    OpenAsSVG,
    Debug,
}

#[expect(clippy::too_many_arguments)]
pub fn doit(
    args: &crate::Args,
    ref_name: Option<&str>,
    dot: Option<Dot>,
    limit: Option<usize>,
    limit_extension: Vec<String>,
    extra_target_spec: Option<&str>,
    hard_limit: Option<usize>,
    no_debug_workspace: bool,
    stats: bool,
    dangerously_skip_postprocessing_for_debugging: bool,
) -> anyhow::Result<()> {
    let mut ctx = but_ctx::Context::discover(&args.current_dir)?;
    let mut repo = ctx.repo.get_mut()?;
    repo.objects.refresh = RefreshMode::Never;
    drop(repo);
    let repo = &*ctx.repo.get()?;
    let extra_target = extra_target_spec
        .map(|rev_spec| repo.rev_parse_single(rev_spec))
        .transpose()?
        .map(|id| id.detach());
    let opts = but_graph::init::Options {
        extra_target_commit_id: extra_target,
        collect_tags: true,
        hard_limit,
        commits_limit_hint: limit,
        commits_limit_recharge_location: limit_extension
            .into_iter()
            .map(|short_hash| {
                repo.objects
                    .lookup_prefix(
                        gix::hash::Prefix::from_hex(&short_hash).expect("valid hex prefix"),
                        None,
                    )
                    .unwrap()
                    .expect("object for prefix exists")
                    .expect("the prefix is unambiguous")
            })
            .collect(),
        dangerously_skip_postprocessing_for_debugging,
    };

    // Never drop - this is read-only.
    let guard = ctx.shared_worktree_access();
    let meta = std::mem::ManuallyDrop::new(ctx.meta(guard.read_permission())?);
    let graph = match ref_name {
        None => but_graph::Graph::from_head(repo, &*meta, opts),
        Some(ref_name) => {
            let mut reference = repo.find_reference(ref_name)?;
            let id = reference.peel_to_id()?;
            but_graph::Graph::from_commit_traversal(id, reference.name().to_owned(), &*meta, opts)
        }
    }?;

    let errors = graph.validation_errors();
    if !errors.is_empty() {
        eprintln!("VALIDATION FAILED: {errors:?}");
    }
    if stats {
        eprintln!("{:#?}", graph.statistics());
    }

    let workspace = graph.to_workspace()?;
    if no_debug_workspace {
        eprintln!(
            "Workspace with {} stacks and {} segments across all stacks with {} commits total",
            workspace.stacks.len(),
            workspace
                .stacks
                .iter()
                .map(|s| s.segments.len())
                .sum::<usize>(),
            workspace
                .stacks
                .iter()
                .flat_map(|s| s.segments.iter().map(|s| s.commits.len()))
                .sum::<usize>(),
        );
    } else {
        eprintln!("{workspace:#?}");
    }

    match dot {
        Some(Dot::Print) => {
            stdout().write_all(graph.dot_graph().as_bytes())?;
        }
        Some(Dot::OpenAsSVG) => {
            #[cfg(unix)]
            graph.open_as_svg();
        }
        Some(Dot::Debug) => {
            eprintln!("{graph:#?}");
        }
        None => {}
    }

    Ok(())
}
