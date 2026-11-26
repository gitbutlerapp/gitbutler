use anyhow::bail;
use but_core::RefMetadata;
use but_error::bail_precondition;
use but_rebase::graph_rebase::{Editor, LookupStep as _, Step, SuccessfulRebase};
use gix::refs::{Category, FullNameRef};

pub fn rename_reference<'ws, 'meta, M: RefMetadata>(
    mut editor: Editor<'ws, 'meta, M>,
    ref_name: &FullNameRef,
    new_name: &FullNameRef,
) -> anyhow::Result<SuccessfulRebase<'ws, 'meta, M>> {
    for name in [ref_name, new_name] {
        if name.category() != Some(Category::LocalBranch) {
            bail!(
                "Can only rename local branches under refs/heads, got '{}'",
                name.as_bstr()
            );
        }
        if but_core::branch::is_technical_branch_name(name.shorten()) {
            bail!("'{}' is a GitButler technical branch", name.shorten());
        }
    }
    let reference = editor.try_select_reference(ref_name).filter(|reference| {
        matches!(
            editor.lookup_step(*reference),
            Ok(Step::Reference { mutable: true, .. })
        )
    });
    let Some(reference) = reference else {
        if editor.repo().try_find_reference(ref_name)?.is_some() {
            bail!(
                "Branch '{}' is outside of what this edit may change",
                ref_name.shorten()
            );
        }
        bail!("Branch '{}' does not exist", ref_name.shorten());
    };
    if editor.try_select_reference(new_name).is_some()
        || editor.repo().try_find_reference(new_name)?.is_some()
    {
        bail_precondition!("A branch named '{}' already exists", new_name.shorten());
    }
    but_core::branch::refuse_colliding_refname(editor.repo(), new_name, Some(ref_name))?;
    editor.replace_reference(reference, new_name.to_owned())?;
    editor.rebase()
}
