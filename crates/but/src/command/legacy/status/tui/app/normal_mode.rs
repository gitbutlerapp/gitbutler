use crate::command::legacy::status::tui::{app::mark::Marks, render::ModeRender};

#[derive(Debug, Default, Clone)]
pub struct NormalMode {
    pub marks: Marks,
}

impl ModeRender for NormalMode {}
