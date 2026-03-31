use crate::command::legacy::status::tui::details::{PartiallyRenderedDiffSection, SectionId};

#[derive(Default, Debug)]
pub(super) struct DetailsCursor {
    selection_section: Option<SectionId>,
}

impl DetailsCursor {
    pub(super) fn move_selection_by<F>(&mut self, sections: &[PartiallyRenderedDiffSection], f: F)
    where
        F: FnOnce(usize) -> usize,
    {
        let Some(selection) = self.selection() else {
            return;
        };
        let Some(current_selection_idx) =
            sections.iter().position(|section| &section.id == selection)
        else {
            return;
        };
        let Some(next) = sections.get(f(current_selection_idx)) else {
            return;
        };
        self.select_section(next.id.clone());
    }

    pub(super) fn select_section(&mut self, id: SectionId) {
        self.selection_section = Some(id);
    }

    pub(super) fn deselect(&mut self) {
        self.selection_section = None;
    }

    pub(super) fn selection(&self) -> Option<&SectionId> {
        self.selection_section.as_ref()
    }
}
