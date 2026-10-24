//! Windows' minimize, maximize and close buttons. The window has no title bar there, as it has
//! none on macOS, so the app draws them over the header's right end, where Windows puts them,
//! and Windows handles their clicks as its own.

use gpui::{
    App, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, Rgba, Styled as _, Window,
    WindowControlArea, div,
};

use super::{HEADER_HEIGHT, Icon, WINDOW_CONTROLS_WIDTH, fixed, h_flex, icon};
use crate::theme::Theme;

/// The buttons, top right, above whatever the window shows, a modal too.
pub fn window_controls(window: &Window, cx: &App) -> impl IntoElement {
    let theme = Theme::get(cx);
    let glyph = theme.text_1;
    // Windows' own red for close, with a white cross.
    let red: Hsla = Rgba {
        r: 232. / 255.,
        g: 17. / 255.,
        b: 35. / 255.,
        a: 1.,
    }
    .into();
    let maximize = match window.is_maximized() {
        true => restore_glyph(glyph),
        false => div().size(fixed(10.)).border_1().border_color(glyph),
    };
    h_flex()
        .absolute()
        .top_0()
        .right_0()
        // Above the header's bottom border, which stays drawn under a hovered button.
        .h(theme.scaled(HEADER_HEIGHT) - fixed(1.))
        .child(caption_button(
            "minimize",
            WindowControlArea::Min,
            theme.hover(),
            div().w(fixed(10.)).h(fixed(1.)).bg(glyph),
        ))
        .child(caption_button(
            "maximize",
            WindowControlArea::Max,
            theme.hover(),
            maximize,
        ))
        .child(
            caption_button("close", WindowControlArea::Close, red, {
                let cross = icon(Icon::Cross).size(fixed(14.)).text_color(glyph);
                cross.group_hover("close", |el| el.text_color(gpui::white()))
            })
            .group("close"),
        )
}

/// Two overlapping squares, the front one whole, as Windows shows "restore".
fn restore_glyph(color: Hsla) -> gpui::Div {
    div()
        .relative()
        .size(fixed(10.))
        .child(
            div()
                .absolute()
                .left_0()
                .bottom_0()
                .size(fixed(8.))
                .border_1()
                .border_color(color),
        )
        .child(
            div()
                .absolute()
                .top_0()
                .right_0()
                .size(fixed(8.))
                .border_t_1()
                .border_r_1()
                .border_color(color),
        )
}

fn caption_button(
    id: &'static str,
    area: WindowControlArea,
    hover: Hsla,
    glyph: impl IntoElement,
) -> gpui::Stateful<gpui::Div> {
    h_flex()
        .id(id)
        .w(fixed(WINDOW_CONTROLS_WIDTH / 3.))
        .h_full()
        .justify_center()
        // The header's drag area is under it; without this, a click there would move the window.
        .occlude()
        .hover(move |el| el.bg(hover))
        .window_control_area(area)
        .child(glyph)
}
