//! A person's picture in a circle, as but-ui-react's `Avatar`: their initial on a colour picked
//! from `seed`, a port of its `personColour.ts`, so a person looks the same here as on but.dev,
//! with their photo over it once it loads.

use gpui::{Div, ImageSource, div, img, prelude::*};

use crate::theme::Theme;
use crate::*;

/// `seed`'s picture `size` across: their initial on their colour, with `picture` over it once it
/// loads, so one that's slow or fails leaves the initial. React draws it at 14 in a compact row,
/// 16 inline and 18 heading a card. Where to fetch a photo from is the app's to know.
pub fn avatar(seed: &str, size: f32, picture: Option<ImageSource>, theme: &Theme) -> Div {
    let people = [
        theme.category_1,
        theme.category_2,
        theme.category_3,
        theme.category_4,
        theme.category_5,
    ];
    let colour = people.get(hash(seed) as usize % people.len());
    div()
        .relative()
        .size(dp(size))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .rounded_full()
        .when_some(colour, |el, colour| el.bg(*colour))
        .text_color(theme.foreground)
        // Over half the circle, so a 14px avatar still holds a readable letter.
        .text_off_scale(size * 0.56)
        .line_height(dp(size))
        .semibold()
        .child(person_initial(seed))
        .when_some(picture, |el, picture| {
            el.child(
                img(picture)
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .rounded_full(),
            )
        })
}

/// `seed`'s first letter or digit, upper-cased, as `personColour.ts`'s `personInitial` gives
/// it: empty when it has neither.
pub fn person_initial(seed: &str) -> String {
    (seed.chars().find(|c| c.is_alphanumeric()))
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default()
}

/// FNV-1a over the UTF-16 units JavaScript reads a string as.
fn hash(seed: &str) -> u32 {
    (seed.encode_utf16()).fold(2_166_136_261, |h, unit| {
        (h ^ u32::from(unit)).wrapping_mul(16_777_619)
    })
}

#[cfg(test)]
mod tests {
    use super::{hash, person_initial};

    #[test]
    fn matches_the_web_apps() {
        // What `personColour.ts` gives for the same names.
        for (seed, colour, letter) in [
            ("Ada Agent", 4, "A"),
            ("Kiril Videlov", 2, "K"),
            ("é😀", 0, "É"),
            ("😀", 2, ""),
        ] {
            assert_eq!(hash(seed) % 5, colour, "{seed} gets the same colour");
            assert_eq!(person_initial(seed), letter, "{seed} gets the same initial");
        }
    }
}
