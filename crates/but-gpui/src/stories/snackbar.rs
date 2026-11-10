use gpui::{Context, IntoElement, Window, prelude::*};

use super::{framed, page, section};
use crate::theme::Theme;
use crate::ui::{snackbar, snackbar_action};

/// What a snackbar says: a plain one and one that went but not quite as asked, each with Undo
/// and without.
pub struct SnackbarStory;

impl Render for SnackbarStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let undo = |id| Some(snackbar_action(id, "Undo", theme).into_any_element());
        page([
            section(
                "Snackbar",
                theme,
                [
                    framed(
                        "Info",
                        theme,
                        snackbar("info", "Committed to main".into(), false, None, theme),
                    ),
                    framed(
                        "With Undo",
                        theme,
                        snackbar(
                            "undo",
                            "Committed to main".into(),
                            false,
                            undo("undo-action"),
                            theme,
                        ),
                    ),
                ],
            ),
            section(
                "Warning",
                theme,
                [
                    framed(
                        "Warning",
                        theme,
                        snackbar(
                            "warning",
                            "Pushed, but the hooks didn't run".into(),
                            true,
                            None,
                            theme,
                        ),
                    ),
                    framed(
                        "With Undo",
                        theme,
                        snackbar(
                            "warning-undo",
                            "Pushed, but the hooks didn't run".into(),
                            true,
                            undo("warning-action"),
                            theme,
                        ),
                    ),
                ],
            ),
        ])
    }
}
