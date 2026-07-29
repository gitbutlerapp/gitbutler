use crate::worktree::checkout::Outcome;

impl std::fmt::Debug for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Outcome { head_update, .. } = self;
        f.debug_struct("Outcome")
            .field(
                "head_update",
                &match head_update {
                    None => "None".to_string(),
                    Some(edits) => edits
                        .last()
                        .map(|edit| {
                            format!("Update {} to {:?}", edit.name, edit.change.new_value())
                        })
                        .unwrap_or_default(),
                },
            )
            .finish()
    }
}
