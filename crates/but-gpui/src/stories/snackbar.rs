use gpui::{Context, IntoElement, Window, prelude::*};

use super::{framed, page, section};
use crate::theme::Theme;
use crate::ui::{
    Icon, SnackbarVariant, snackbar, snackbar_dismiss, snackbar_undo, snackbar_with_icon,
};

/// Each variant as Figma draws it: on its own, with Undo, and waiting to be dismissed.
pub struct SnackbarStory;

const VARIANTS: [(&str, SnackbarVariant, &str); 4] = [
    ("Info", SnackbarVariant::Info, "Committed to main"),
    (
        "Warning",
        SnackbarVariant::Warning,
        "Pushed, but the hooks didn't run",
    ),
    ("Danger", SnackbarVariant::Danger, "Couldn't push to origin"),
    ("Safe", SnackbarVariant::Safe, "Merged into main"),
];

impl Render for SnackbarStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        page(
            VARIANTS
                .into_iter()
                .enumerate()
                .map(|(ix, (name, variant, said))| {
                    let undo = snackbar_undo(("undo", ix), theme).into_any_element();
                    let dismiss = snackbar_dismiss(("dismiss", ix), theme).into_any_element();
                    section(
                        name,
                        theme,
                        [
                            framed(
                                "Plain",
                                theme,
                                snackbar(("plain", ix), said.into(), variant, None, theme),
                            ),
                            framed(
                                "With Undo",
                                theme,
                                snackbar(
                                    ("with-undo", ix),
                                    said.into(),
                                    variant,
                                    Some(undo),
                                    theme,
                                ),
                            ),
                            framed(
                                "Dismissable",
                                theme,
                                snackbar(
                                    ("dismissable", ix),
                                    said.into(),
                                    variant,
                                    Some(dismiss),
                                    theme,
                                ),
                            ),
                        ],
                    )
                })
                .chain([section(
                    "Info with a glyph, when it needs one",
                    theme,
                    [framed(
                        "With an icon",
                        theme,
                        snackbar_with_icon(
                            "info-icon",
                            "Copied the branch name".into(),
                            SnackbarVariant::Info,
                            Icon::Info,
                            None,
                            theme,
                        ),
                    )],
                )]),
        )
    }
}
