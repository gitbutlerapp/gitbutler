//! The window's minimize, maximize and close buttons, where the window has no title bar to
//! carry them: on Windows, and on Linux where the desktop leaves the frame to the app, as GNOME
//! does on Wayland. The app draws them over the header's right end. Windows handles their clicks
//! as its own; on Linux the app does.

use gpui::{App, Hsla, Rgba, Window, WindowControlArea, div, prelude::*};

use super::{HEADER_HEIGHT, Icon, fixed, h_flex, icon};
use crate::theme::Theme;

const LINUX: bool = cfg!(target_os = "linux");
/// GNOME's round buttons: how large they are, and the room between and beside them.
const ROUND: f32 = 24.;
const ROUND_GAP: f32 = 12.;

/// How much of the header's right end the buttons cover: three as wide as Windows draws its
/// own, or GNOME's round ones with room around them.
pub const WINDOW_CONTROLS_WIDTH: f32 = match LINUX {
    true => 3. * ROUND + 4. * ROUND_GAP,
    false => 3. * 46.,
};

/// The buttons, top right, above whatever the window shows, a modal too.
pub fn window_controls(window: &Window, cx: &App) -> impl IntoElement {
    let theme = Theme::get(cx);
    let glyph = theme.text_1;
    // Windows' own red for close, with a white cross; GNOME's is like the others.
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
    let cross = icon(Icon::Cross).size(fixed(14.)).text_color(glyph);
    let hover = match LINUX {
        true => theme.border_2,
        false => theme.hover(),
    };
    h_flex()
        .absolute()
        .top_0()
        .right_0()
        // Above the header's bottom border, which stays drawn under a hovered button.
        .h(theme.scaled(HEADER_HEIGHT) - fixed(1.))
        .when(LINUX, |el| el.px(fixed(ROUND_GAP)).gap(fixed(ROUND_GAP)))
        .child(caption_button(
            "minimize",
            WindowControlArea::Min,
            theme,
            hover,
            div().w(fixed(10.)).h(fixed(1.)).bg(glyph),
        ))
        .child(caption_button(
            "maximize",
            WindowControlArea::Max,
            theme,
            hover,
            maximize,
        ))
        .child(match LINUX {
            true => caption_button("close", WindowControlArea::Close, theme, hover, cross),
            false => caption_button(
                "close",
                WindowControlArea::Close,
                theme,
                red,
                cross.group_hover("close", |el| el.text_color(gpui::white())),
            )
            .group("close"),
        })
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
    theme: &Theme,
    hover: Hsla,
    glyph: impl IntoElement,
) -> gpui::Stateful<gpui::Div> {
    h_flex()
        .id(id)
        .justify_center()
        // The header's drag area is under it; without this, a click there would move the window.
        .occlude()
        .window_control_area(area)
        .hover(move |el| el.bg(hover))
        .map(|el| match LINUX {
            // GNOME's: round, on a faint ground.
            true => el
                .size(fixed(ROUND))
                .rounded_full()
                .bg(theme.hover())
                .on_click(move |_, window, cx| match area {
                    WindowControlArea::Min => window.minimize_window(),
                    WindowControlArea::Max => window.zoom_window(),
                    // Asks first, as closing the window does elsewhere.
                    _ => window.dispatch_action(Box::new(crate::app::Quit), cx),
                }),
            false => el.w(fixed(WINDOW_CONTROLS_WIDTH / 3.)).h_full(),
        })
        .child(glyph)
}
