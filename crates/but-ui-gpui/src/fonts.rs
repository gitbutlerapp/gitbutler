//! The fonts the components are set in, compiled in.

use std::borrow::Cow;

use crate::*;

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

/// How wide a digit of `family` is in ems, or `None` when it isn't installed and GPUI would
/// quietly fall back.
pub fn digit_advance(family: &str, text_system: &gpui::TextSystem) -> Option<f32> {
    let id = text_system.resolve_font(&gpui::font(family.to_owned()));
    (text_system.get_font_for_id(id)?.family == family).then_some(())?;
    let size = fixed(100.);
    let advance = text_system.advance(id, size, '0').ok()?;
    Some(advance.width / size)
}

/// Match the faces against the installed fonts in the background, as Zed does: cosmic-text
/// otherwise walks every installed font on the main thread the first time it uses a face.
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

#[cfg(test)]
mod tests {
    use crate::{FONT_WEIGHT_REGULAR, FONT_WEIGHT_SEMIBOLD};

    /// How much lighter `assets/fonts/cut-inter.py` cuts a file than the weight it stands in for.
    const OFFSET: f32 = 60.;

    /// The weight `font` was cut at and the weight it stands in for, from the Description
    /// record (name ID 10) the script writes: `Cut at wght 520 for --but-font-weight-semibold, 580`.
    fn cut(font: &[u8]) -> Option<(f32, f32)> {
        let u16_at = |at: usize| Some(u16::from_be_bytes(font.get(at..at + 2)?.try_into().ok()?));
        let u32_at = |at: usize| Some(u32::from_be_bytes(font.get(at..at + 4)?.try_into().ok()?));
        let name = (0..usize::from(u16_at(4)?))
            .map(|table| 12 + table * 16)
            .find(|&at| font.get(at..at + 4) == Some(b"name".as_slice()))?;
        let base = usize::try_from(u32_at(name + 8)?).ok()?;
        let strings = base + usize::from(u16_at(base + 4)?);
        let description = (0..usize::from(u16_at(base + 2)?))
            .map(|record| base + 6 + record * 12)
            .find_map(|at| {
                (u16_at(at)? == 3 && u16_at(at + 6)? == 10).then_some(())?;
                let start = strings + usize::from(u16_at(at + 10)?);
                let bytes = font.get(start..start + usize::from(u16_at(at + 8)?))?;
                let units: Vec<u16> = (bytes.as_chunks::<2>().0.iter())
                    .map(|&pair| u16::from_be_bytes(pair))
                    .collect();
                String::from_utf16(&units).ok()
            })?;
        let at = description
            .strip_prefix("Cut at wght ")?
            .split(' ')
            .next()?;
        let weight = description.rsplit(", ").next()?;
        Some((at.parse().ok()?, weight.parse().ok()?))
    }

    #[test]
    fn each_inter_file_is_cut_for_its_weight_token() {
        let files: [(&str, &[u8], f32); 3] = [
            (
                "Inter-Regular.ttf",
                include_bytes!("../assets/fonts/Inter-Regular.ttf"),
                FONT_WEIGHT_REGULAR,
            ),
            (
                "Inter-Medium.ttf",
                include_bytes!("../assets/fonts/Inter-Medium.ttf"),
                500.,
            ),
            (
                "Inter-SemiBold.ttf",
                include_bytes!("../assets/fonts/Inter-SemiBold.ttf"),
                FONT_WEIGHT_SEMIBOLD,
            ),
        ];
        for (file, font, weight) in files {
            assert_eq!(
                cut(font),
                Some((weight - OFFSET, weight)),
                "{file} stands in for {weight}, so it's cut at {}: run assets/fonts/cut-inter.py",
                weight - OFFSET
            );
        }
    }
}
