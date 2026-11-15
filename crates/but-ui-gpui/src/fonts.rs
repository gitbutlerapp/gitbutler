//! The fonts the components are set in, compiled in.

use std::borrow::Cow;

use crate::*;

/// GitButler's fonts, compiled into the binary so the client looks the same everywhere.
pub fn fonts() -> Vec<Cow<'static, [u8]>> {
    macro_rules! fonts {
        ($($name:literal),* $(,)?) => {
            vec![$(Cow::Borrowed(include_bytes!(concat!("../assets/fonts/", $name)).as_slice()),)*]
        };
    }
    fonts!(
        "Inter-Regular.ttf",
        "Inter-Medium.ttf",
        "Inter-SemiBold.ttf",
        "GeistMonoNerdFontMono-Regular.otf",
        "GeistMonoNerdFontMono-SemiBold.otf",
    )
}

/// How wide a digit of `family` is in ems, or `None` when the machine hasn't got it, as GPUI
/// would quietly draw it in its own fallback.
pub fn digit_advance(family: &str, text_system: &gpui::TextSystem) -> Option<f32> {
    let id = text_system.resolve_font(&gpui::font(family.to_owned()));
    (text_system.get_font_for_id(id)?.family == family).then_some(())?;
    let size = fixed(100.);
    let advance = text_system.advance(id, size, '0').ok()?;
    Some(advance.width / size)
}

/// Match each of `fonts`' faces against the installed fonts in the background, as Zed does.
/// cosmic-text otherwise does it on the main thread the first time it lays out text in a face,
/// and it walks every installed font to do so.
#[cfg(target_os = "linux")]
pub fn prewarm_fonts(cx: &gpui::App) {
    use gpui::{AppContext as _, FontWeight};

    let fonts = [
        ("Inter", FontWeight::NORMAL),
        ("Inter", FontWeight::MEDIUM),
        ("Inter", FontWeight::SEMIBOLD),
        ("GeistMono Nerd Font Mono", FontWeight::NORMAL),
        ("GeistMono Nerd Font Mono", FontWeight::SEMIBOLD),
    ]
    .map(|(family, weight)| gpui::Font {
        weight,
        ..gpui::font(family)
    });
    let text_system = cx.text_system().clone();
    cx.background_spawn(async move { text_system.prewarm_fonts(&fonts) })
        .detach();
}
