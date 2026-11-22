//! Where a person's photo comes from: their GitHub or Gravatar picture, drawn by
//! [`super::avatar`] over their initial.

use std::sync::{Arc, OnceLock};

use gpui::{
    AnyElement, App, Asset, Div, Image, ImageCacheError, ImageSource, SharedString, Window,
    prelude::*,
};

use super::dp;
use crate::backend::Agent;
use crate::backend::accounts::GitButlerUser;
use crate::theme::Theme;

/// `user`'s `picture`, or without one their initial, as in GitButler's other apps.
pub fn account_avatar(
    user: &GitButlerUser,
    picture: Option<Arc<Image>>,
    size: f32,
    theme: &Theme,
) -> AnyElement {
    let seed = (user.login.as_ref().or(user.email.as_ref())).unwrap_or(&user.name);
    super::avatar(seed, size, picture.map(ImageSource::Image), theme).into_any_element()
}

/// `seed`'s picture: their initial on their colour, in a circle `size` across.
pub fn person_avatar(seed: &str, size: f32, theme: &Theme) -> Div {
    super::avatar(seed, size, None, theme)
}

/// `name`'s picture, with the photo for `email` over their initial once it loads. Someone
/// without one, or offline, keeps the initial.
pub fn author_avatar(name: &str, email: &str, size: f32, theme: &Theme) -> Div {
    let email = SharedString::from(email.trim().to_lowercase());
    super::avatar(name, size, Some(photo::<Photo>(email)), theme)
}

/// A forge user's picture: the one at `url` over their initial once it loads.
pub fn forge_avatar(login: &str, url: Option<&str>, size: f32, theme: &Theme) -> Div {
    let picture = url.map(|url| photo::<Picture>(url.to_owned().into()));
    super::avatar(login, size, picture, theme)
}

/// The photo `A` loads for `key`, for [`super::avatar`] to draw over the initial once there is
/// one.
fn photo<A>(key: SharedString) -> ImageSource
where
    A: Asset<Source = SharedString, Output = Option<Arc<gpui::Image>>>,
{
    ImageSource::Custom(Arc::new(
        move |window: &mut Window, cx: &mut App| match window.use_asset::<A>(&key, cx)? {
            Some(photo) => photo.use_render_image(window, cx).map(Ok),
            None => Some(Err(ImageCacheError::Asset("No photo".into()))),
        },
    ))
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
            .border_color(theme.background),
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
