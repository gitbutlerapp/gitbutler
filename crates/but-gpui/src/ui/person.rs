//! A person without a picture, as GitButler's other apps draw them: a pattern in a colour,
//! both picked from their name. A port of `@gitbutler/ui-react`'s `personColour.ts`, so an
//! author looks the same here as on but.dev. An author with a GitHub or Gravatar photo gets it
//! on top.

use std::fmt::Write;
use std::sync::{Arc, OnceLock};

use gpui::{
    App, Asset, Div, ImageCacheError, ImageSource, SharedString, Window, div, img, prelude::*, svg,
};

use super::dp;
use crate::backend::Agent;
use crate::theme::Theme;

/// The pattern's grid, in blocks across.
const SIZE: usize = 8;

/// Where a pattern is served from, by the number it's drawn from, for [`gpui::svg`].
const PREFIX: &str = "person/";

/// `seed`'s picture: their pattern on their colour, in a circle `size` across.
pub fn person_avatar(seed: &str, size: f32, theme: &Theme) -> Div {
    let Some(&[ground, shade]) = theme.person.get(hash(seed) as usize % theme.person.len()) else {
        return div();
    };
    let pattern = hash(&format!("{seed}#glitch"));
    div()
        .size(dp(size))
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

/// `name`'s picture, with the photo for `email` over their pattern once it loads. Someone
/// without one, or offline, keeps the pattern.
pub fn author_avatar(name: &str, email: &str, size: f32, theme: &Theme) -> Div {
    let email = SharedString::from(email.trim().to_lowercase());
    with_photo::<Photo>(person_avatar(name, size, theme), email)
}

/// A forge user's picture: the one at `url` over their pattern once it loads.
pub fn forge_avatar(login: &str, url: Option<&str>, size: f32, theme: &Theme) -> Div {
    let pattern = person_avatar(login, size, theme);
    match url {
        Some(url) => with_photo::<Picture>(pattern, url.to_owned().into()),
        None => pattern,
    }
}

/// `pattern` with the photo `A` loads for `key` over it, once there is one.
fn with_photo<A>(pattern: Div, key: SharedString) -> Div
where
    A: Asset<Source = SharedString, Output = Option<Arc<gpui::Image>>>,
{
    let photo = move |window: &mut Window, cx: &mut App| match window.use_asset::<A>(&key, cx)? {
        Some(photo) => photo.use_render_image(window, cx).map(Ok),
        None => Some(Err(ImageCacheError::Asset("No photo".into()))),
    };
    pattern.relative().child(
        img(ImageSource::Custom(Arc::new(photo)))
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .rounded_full(),
    )
}

/// `name`'s picture, as [`author_avatar`] draws it, with `agent`'s small at its corner when an
/// agent wrote what they committed.
pub fn byline_avatar(
    name: &str,
    email: &str,
    agent: Option<&Agent>,
    size: f32,
    theme: &Theme,
) -> Div {
    let person = author_avatar(name, email, size, theme);
    let Some(agent) = agent else {
        return person;
    };
    let small = (size * 0.6).round();
    person.child(
        author_avatar(&agent.name, &agent.email, small, theme)
            .absolute()
            .right(dp(-small / 3.))
            .bottom(dp(-small / 3.))
            .border_1()
            .border_color(theme.bg_1),
    )
}

/// Who it's by and when, as a page's header says it: `name`'s picture, with `agent`'s, and
/// "Ada with Claude at" `time`.
pub fn byline(name: &str, email: &str, agent: Option<&Agent>, time: i64, theme: &Theme) -> Div {
    let by = match agent {
        Some(agent) => format!("{name} with {}", agent.name),
        None => name.to_owned(),
    };
    super::h_flex()
        .gap(dp(8.))
        .child(byline_avatar(name, email, agent, 16., theme))
        .child(format!("{by} at {}", super::local_time(time)))
}

/// The photo for a lowercased email, fetched once a run, off the UI thread: the GitHub
/// picture of the account it belongs to, else the Gravatar one.
struct Photo;

impl Asset for Photo {
    type Source = SharedString;
    type Output = Option<Arc<gpui::Image>>;

    fn load(
        email: SharedString,
        _: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let photo = crate::backend::run(move || Ok(github(&email).or_else(|| gravatar(&email))));
        async move { photo.await.ok().flatten() }
    }
}

/// The picture at a URL, fetched once a run, off the UI thread.
struct Picture;

impl Asset for Picture {
    type Source = SharedString;
    type Output = Option<Arc<gpui::Image>>;

    fn load(url: SharedString, _: &mut App) -> impl Future<Output = Self::Output> + Send + 'static {
        let photo = crate::backend::run(move || Ok(crate::backend::accounts::avatar(&url).ok()));
        async move { photo.await.ok().flatten() }
    }
}

/// The picture of the GitHub account `email` belongs to. GitHub answers an address no
/// account has with its default picture, which, read once a run from an address none can
/// have, counts as none; until it's been read, as while offline, neither can a photo.
fn github(email: &str) -> Option<Arc<gpui::Image>> {
    use crate::backend::accounts::{fetch, picture};
    static DEFAULT: OnceLock<md5::Digest> = OnceLock::new();
    let url = |email: &str| {
        let base = "https://avatars.githubusercontent.com/u/e";
        reqwest::Url::parse_with_params(base, [("email", email), ("s", "64")]).ok()
    };
    let default = match DEFAULT.get() {
        Some(default) => *default,
        None => {
            // Made up anew each run: GitHub has a picture of its own for some made-up
            // addresses, as for nobody@users.invalid, and with it every author counted as
            // having a photo, all of them GitHub's default.
            let nobody = format!("nobody-{}@users.invalid", std::process::id());
            let bytes = fetch(url(&nobody)?.as_str()).ok()?;
            *DEFAULT.get_or_init(|| md5::compute(bytes))
        }
    };
    let bytes = fetch(url(email)?.as_str()).ok()?;
    (md5::compute(&bytes) != default)
        .then(|| picture(bytes).ok())
        .flatten()
}

/// The Gravatar photo for `email`. `d=404` asks for a real photo only, so someone without one
/// gets `None` rather than a generated face.
fn gravatar(email: &str) -> Option<Arc<gpui::Image>> {
    let hash = md5::compute(email.as_bytes());
    let url = format!("https://www.gravatar.com/avatar/{hash:x}?s=64&r=g&d=404");
    crate::backend::accounts::avatar(&url).ok()
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
#[expect(
    clippy::indexing_slicing,
    reason = "`Random::int(n)` is below `n`, and the grid's arithmetic keeps indexes in the arrays"
)]
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
