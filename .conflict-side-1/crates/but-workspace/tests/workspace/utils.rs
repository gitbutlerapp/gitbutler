use std::borrow::Cow;

use but_core::{DiffSpec, TreeStatus};

pub const CONTEXT_LINES: u32 = 0;

pub use but_testsupport::{
    read_only_in_memory_scenario, read_only_in_memory_scenario_named, visualize_index,
    writable_scenario, writable_scenario_slow, writable_scenario_with_args,
};

/// Always use all the hunks.
pub fn to_change_specs_whole_file(changes: but_core::WorktreeChanges) -> Vec<DiffSpec> {
    let out: Vec<_> = changes
        .changes
        .into_iter()
        .map(|change| DiffSpec {
            previous_path: change.previous_path().map(ToOwned::to_owned),
            path: change.path,
            hunk_headers: Vec::new(),
        })
        .collect();
    assert!(
        !out.is_empty(),
        "fixture should contain actual changes to turn into requests"
    );
    out
}

/// Always use all the hunks.
pub fn to_change_specs_all_hunks(
    repo: &gix::Repository,
    changes: but_core::WorktreeChanges,
) -> anyhow::Result<Vec<DiffSpec>> {
    to_change_specs_all_hunks_with_context_lines(repo, changes, CONTEXT_LINES)
}

/// Always use all the hunks.
pub fn to_change_specs_all_hunks_with_context_lines(
    repo: &gix::Repository,
    changes: but_core::WorktreeChanges,
    context_lines: u32,
) -> anyhow::Result<Vec<DiffSpec>> {
    let mut out = Vec::with_capacity(changes.changes.len());
    for change in changes.changes {
        let spec = match change.status {
            // Untracked files must always be taken from disk (they don't have a counterpart in a tree yet)
            TreeStatus::Addition { is_untracked, .. } if is_untracked => DiffSpec {
                path: change.path,
                ..Default::default()
            },
            _ => {
                match change.unified_patch(repo, context_lines)? {
                    Some(but_core::UnifiedPatch::Patch { hunks, .. }) => DiffSpec {
                        previous_path: change.previous_path().map(ToOwned::to_owned),
                        path: change.path,
                        hunk_headers: hunks.into_iter().map(Into::into).collect(),
                    },
                    Some(_) => unreachable!("tests won't be binary or too large"),
                    None => {
                        // Assume it's a submodule or something without content, don't do hunks then.
                        DiffSpec {
                            path: change.path,
                            ..Default::default()
                        }
                    }
                }
            }
        };
        out.push(spec);
    }
    Ok(out)
}

pub fn r(name: &str) -> &gix::refs::FullNameRef {
    name.try_into().expect("statically known valid ref-name")
}

pub fn rc(name: &str) -> Cow<'static, gix::refs::FullNameRef> {
    Cow::Owned(name.try_into().expect("statically known valid ref-name"))
}
