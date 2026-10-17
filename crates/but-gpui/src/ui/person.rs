//! A person without a picture, as GitButler's other apps draw them: a pattern in a colour,
//! both picked from their name. A port of `@gitbutler/ui-react`'s `personColour.ts`, so an
//! author looks the same here as on but.dev.

use std::fmt::Write;

use gpui::{Div, div, prelude::*, px, svg};

use crate::theme::Theme;

/// The pattern's grid, in blocks across.
const SIZE: usize = 8;

/// Where a pattern is served from, by the number it's drawn from, for [`gpui::svg`].
const PREFIX: &str = "person/";

/// `seed`'s picture: their pattern on their colour, in a circle `size` across.
pub fn person_avatar(seed: &str, size: f32, theme: &Theme) -> Div {
    let [ground, shade] = theme.person[hash(seed) as usize % theme.person.len()];
    let pattern = hash(&format!("{seed}#glitch"));
    div()
        .size(px(size))
        .flex_none()
        .rounded_full()
        .bg(ground)
        .child(
            svg()
                .path(format!("{PREFIX}{pattern:08x}.svg"))
                .size_full()
                .text_color(shade),
        )
}

/// The SVG of a pattern, by its asset path: one colour, clipped to the circle it's shown in.
pub(super) fn load(path: &str) -> Option<Vec<u8>> {
    let hex = path.strip_prefix(PREFIX)?.strip_suffix(".svg")?;
    let d = pattern(u32::from_str_radix(hex, 16).ok()?);
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {SIZE} {SIZE}"><clipPath id="c"><circle cx="4" cy="4" r="4"/></clipPath><path clip-path="url(#c)" d="{d}"/></svg>"#
    );
    Some(svg.into_bytes())
}

/// FNV-1a over the UTF-16 units JavaScript reads a string as.
fn hash(seed: &str) -> u32 {
    (seed.encode_utf16()).fold(2_166_136_261, |h, unit| {
        (h ^ u32::from(unit)).wrapping_mul(16_777_619)
    })
}

/// mulberry32: the same numbers for the same seed as the web apps get.
struct Random(u32);

impl Random {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let s = self.0;
        let mut t = (s ^ (s >> 15)).wrapping_mul(1 | s);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        f64::from(t ^ (t >> 14)) / 4_294_967_296.
    }

    /// A whole number below `n`.
    fn int(&mut self, n: usize) -> usize {
        (self.next() * n as f64) as usize
    }
}

/// The pattern drawn from `seed` as an SVG path on a `SIZE` square, symmetric about its
/// vertical middle: bands of rows, each its own width, growing from the middle out, and a stray
/// block or two.
fn pattern(seed: u32) -> String {
    let mut r = Random(seed);
    let half = SIZE / 2;
    // The left half only, row by row; the right is its mirror.
    let mut cells = [false; SIZE * SIZE / 2];
    let mut y = 0;
    while y < SIZE {
        let band = 1 + r.int(3);
        let width = [1, 1, 2, 2, 3, 3][r.int(6)];
        for row in y..(y + band).min(SIZE) {
            for k in 0..width {
                cells[row * half + half - 1 - k] = true;
            }
        }
        y += band;
    }
    for _ in 0..1 + r.int(2) {
        let i = r.int(SIZE) * half + r.int(half);
        cells[i] = !cells[i];
    }
    let upside_down = r.next() < 0.5;
    let at = |x: usize, y: usize| {
        let row = if upside_down { SIZE - 1 - y } else { y };
        cells[row * half + if x < half { x } else { SIZE - 1 - x }]
    };
    // Runs of filled blocks along each row.
    let mut d = String::new();
    for row in 0..SIZE {
        let mut x = 0;
        while x < SIZE {
            if !at(x, row) {
                x += 1;
                continue;
            }
            let start = x;
            while x < SIZE && at(x, row) {
                x += 1;
            }
            let run = x - start;
            write!(d, "M{start} {row}h{run}v1h-{run}z").ok();
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::{hash, pattern};

    #[test]
    fn matches_the_web_apps() {
        // What `personColour.ts` gives for the same names.
        for (seed, colour, d) in [
            (
                "Ada Agent",
                4,
                "M3 0h2v1h-2zM3 1h2v1h-2zM1 2h6v1h-6zM0 3h8v1h-8zM1 4h6v1h-6zM1 5h6v1h-6zM1 6h6v1h-6zM1 7h6v1h-6z",
            ),
            (
                "Kiril Videlov",
                2,
                "M3 0h2v1h-2zM3 1h2v1h-2zM3 2h2v1h-2zM3 3h2v1h-2zM2 4h4v1h-4zM2 5h4v1h-4zM0 6h1v1h-1zM2 6h4v1h-4zM7 6h1v1h-1zM1 7h6v1h-6z",
            ),
            (
                "é😀",
                0,
                "M3 0h2v1h-2zM2 1h4v1h-4zM2 2h4v1h-4zM2 3h4v1h-4zM3 4h2v1h-2zM2 5h4v1h-4zM2 6h4v1h-4zM3 7h2v1h-2z",
            ),
        ] {
            assert_eq!(hash(seed) % 5, colour, "{seed} gets the same colour");
            let drawn = pattern(hash(&format!("{seed}#glitch")));
            assert_eq!(drawn, d, "{seed} gets the same pattern");
        }
    }
}
