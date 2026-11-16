//! Tooltips, as but-ui-react's `Tooltip`.

use gpui::{App, Pixels, Point, SharedString, Window, div, prelude::*};

use crate::theme::{Theme, tokens};
use crate::*;

pub struct Tooltip {
    text: SharedString,
    key: Option<SharedString>,
    /// When it first showed: what had covered the window by then, and where the pointer was.
    shown: Option<(u64, Point<Pixels>)>,
}

/// What covers the window, as a menu, picker, prompt or modal does: how many times one came up,
/// and where the pointer was when the current one did. gpui keeps a tooltip shown for an element
/// under it until the pointer leaves the element; this tells the tooltip to hide meanwhile.
#[derive(Default)]
pub struct Covered {
    times: u64,
    at: Option<Point<Pixels>>,
}

impl gpui::Global for Covered {}

impl Covered {
    pub fn note(covered: bool, window: &Window, cx: &mut App) {
        let state = cx.default_global::<Covered>();
        match (covered, state.at) {
            (true, None) => {
                state.times += 1;
                state.at = Some(window.mouse_position());
            }
            (false, Some(_)) => state.at = None,
            _ => {}
        }
    }
}

impl Tooltip {
    pub fn keyed(
        text: impl Into<SharedString>,
        key: Option<SharedString>,
    ) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView + 'static {
        let text = text.into();
        move |_, cx| {
            let (text, key) = (text.clone(), key.clone());
            cx.new(|_| Tooltip {
                text,
                key,
                shown: None,
            })
            .into()
        }
    }

    pub fn text(
        text: impl Into<SharedString>,
    ) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView + 'static {
        Self::keyed(text, None)
    }
}

impl Render for Tooltip {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let (times, at) = cx
            .try_global::<Covered>()
            .map_or((0, None), |c| (c.times, c.at));
        let shown = *self.shown.get_or_insert((times, window.mouse_position()));
        // An element's under what covers the window when its tooltip showed before that came,
        // or since without the pointer moving onto something on top.
        let under = at.is_some_and(|at| shown.0 != times || shown.1 == at);
        let theme = Theme::get(cx);
        popover(theme)
            .when(under, |el| el.invisible())
            .flex_row()
            .items_center()
            .mt(dp(4.))
            .max_w(dp(tokens::SIZE_TOOLTIP_MAX))
            .rounded(dp(theme.shape.control))
            .pl(dp(tokens::PADDING_TOOLTIP_INLINE))
            // After a shortcut, the block padding, as but-ui's does.
            .pr(dp(match self.key.is_some() {
                true => tokens::PADDING_TOOLTIP_BLOCK,
                false => tokens::PADDING_TOOLTIP_INLINE,
            }))
            .py(dp(tokens::PADDING_TOOLTIP_BLOCK))
            .text_12()
            .text_color(theme.text_1)
            .font_family(theme.ui_font.clone())
            .gap(dp(tokens::GAP_TOOLTIP))
            // A long one, even a path, wraps within the cap.
            .child(div().min_w_0().child(self.text.clone()))
            .when_some(self.key.clone(), |el, key| {
                el.child(keycap(&key, theme).flex_none())
            })
    }
}
