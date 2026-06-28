use crate::command::legacy::status::tui::Marks;

#[derive(Debug, Default, Clone)]
pub struct PickChangesMode {
    pub marks: Marks,
}
