use gpui::{Context, Window, prelude::*};

use super::{page, section, variant};
use crate::theme::Theme;
use crate::ui::keycap;

/// Keys written as macOS writes them, which `keycap` shows as Ctrl and Shift elsewhere.
const KEYS: [&str; 5] = ["⌘K", "⇧⌘P", "⌘↩", "⌥⌘Z", "Esc"];

pub struct KeycapStory;

impl Render for KeycapStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        page([section(
            "Keys",
            theme,
            KEYS.map(|keys| variant(keys, theme, keycap(keys, theme))),
        )])
    }
}
