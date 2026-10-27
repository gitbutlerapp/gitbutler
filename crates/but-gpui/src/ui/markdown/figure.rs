//! An image a description draws: fetched from GitHub as it's first drawn, off the UI thread,
//! and kept while it's drawn, and after while the images kept are few. Until it's fetched, a box holds its place, its size
//! when the HTML gives one, and an image that can't be had shows its words instead.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use gpui::{
    App, Asset, ImageSource, RenderImage, Role, SharedString, Window, div, img, prelude::*,
    relative,
};
use parking_lot::Mutex;

use super::{Image, words};
use crate::backend::{self, Picture};
use crate::theme::Theme;
use crate::ui::dp;

/// About how many bytes of pixels the images drawn lately keep, some twenty screenshots;
/// past that the oldest go, and are fetched again if they're drawn again.
const KEPT_BYTES: usize = 160 << 20;

/// How tall a box holds an image's place until it's fetched, when the HTML doesn't say.
const PLACE_HEIGHT: f32 = 160.;

/// An image of a description, which a click previews, with the rest of its `gallery`, the
/// description's images, a key away.
#[derive(IntoElement)]
pub(super) struct Figure {
    id: SharedString,
    gallery: Arc<[Image]>,
    at: usize,
}

impl Figure {
    pub(super) fn new(id: impl Into<SharedString>, gallery: Arc<[Image]>, at: usize) -> Self {
        Self {
            id: id.into(),
            gallery,
            at,
        }
    }
}

/// The picture at `src`, once fetched, counted as drawn in this frame.
pub(super) fn fetched(src: &str, window: &mut Window, cx: &mut App) -> Option<Option<Picture>> {
    let src = SharedString::from(src.to_owned());
    // Once fetched, the view drawing it draws again.
    let fetched = window.use_asset::<Fetched>(&src, cx);
    if let Some(Some(picture)) = &fetched {
        keep(&src, picture, cx);
    }
    fetched
}

impl RenderOnce for Figure {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Figure { id, gallery, at } = self;
        let Some(image) = gallery.get(at).cloned() else {
            return div().into_any_element();
        };
        let src = SharedString::from(image.src.clone());
        let fetched = fetched(&image.src, window, cx);
        let theme = Theme::get(cx);
        let place = div()
            .id(id.clone())
            .relative()
            .max_w_full()
            .role(Role::Button)
            .aria_label(image.words())
            .on_click(move |_, window, cx| super::preview::open(gallery.clone(), at, window, cx));
        // A hairline over its edge, so a screenshot as pale as the page still has one.
        let edge = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .rounded(dp(6.))
            .border_1()
            .border_color(theme.border_3);
        // As GitHub sizes it: by its HTML, else a point a pixel, never wider than there's
        // room for. Until it lands, a box holds its place, exactly when the HTML gives its
        // size or it was fetched earlier this run.
        let natural = match &fetched {
            Some(Some(picture)) => Some((picture.width, picture.height)),
            _ => known_size(&src),
        };
        let sized = match (image.width, image.height, natural) {
            (Some(width), Some(height), _) => Some((width, width / height)),
            (width, height, Some((across, down))) => {
                let ratio = across / down.max(1.);
                Some((width.or(height.map(|h| h * ratio)).unwrap_or(across), ratio))
            }
            _ => None,
        };
        let place = match sized {
            Some((width, ratio)) => place.w(dp(width)).aspect_ratio(ratio),
            None => place
                .w(image.width.map_or(relative(1.), |width| dp(width).into()))
                .h(dp(PLACE_HEIGHT)),
        };
        match fetched {
            Some(Some(picture)) => place
                .child(
                    img(ImageSource::Render(picture.image))
                        .size_full()
                        .rounded(dp(6.)),
                )
                .child(edge)
                .into_any_element(),
            None => (place.bg(theme.fill_gray.opacity(0.06)).child(edge)).into_any_element(),
            Some(None) => words(&id, &image, theme),
        }
    }
}

/// An image on GitHub, fetched once a run, or `None` when it can't be had: offline, gone, or
/// in a private repository no GitHub account added can see.
struct Fetched;

impl Asset for Fetched {
    type Source = SharedString;
    type Output = Option<Picture>;

    fn load(
        url: SharedString,
        cx: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        // A second after it lands, when its description has drawn it if it's still shown,
        // what isn't on screen is let go: paging on, nothing else may draw to let it go.
        let (landed, lands) = futures::channel::oneshot::channel::<()>();
        cx.spawn(async move |cx| {
            if lands.await.is_ok() {
                cx.background_executor().timer(LANDED_PASS).await;
                cx.update(|cx| let_go(cx, false));
            }
        })
        .detach();
        let picture = backend::run({
            let url = url.clone();
            move || backend::images::picture(&url)
        });
        async move {
            let picture = (picture.await)
                .inspect_err(|err| tracing::debug!("Couldn't show an image: {err:#}"))
                .ok()?;
            KEPT.lock().landed(&url, &picture);
            landed.send(()).ok();
            Some(picture)
        }
    }
}

/// How long after an image lands what isn't on screen is let go.
const LANDED_PASS: Duration = Duration::from_secs(1);

/// The images fetched, by where they're from, the one drawn longest ago first, each with the
/// frame it was last drawn in, or none for one that landed and wasn't drawn since.
#[derive(Default)]
struct Kept {
    images: VecDeque<(SharedString, Option<u64>, Arc<RenderImage>)>,
    /// The frame being drawn, counted by the passes after them.
    frame: u64,
    /// Whether a pass is to come after this frame.
    pass: bool,
    /// How big each image fetched this run is, so one fetched again holds its place exactly
    /// and moves nothing as it lands.
    sizes: HashMap<SharedString, (f32, f32)>,
}

static KEPT: LazyLock<Mutex<Kept>> = LazyLock::new(Default::default);

impl Kept {
    fn landed(&mut self, url: &SharedString, picture: &Picture) {
        (self.sizes).insert(url.clone(), (picture.width, picture.height));
        if !self.images.iter().any(|(kept, ..)| kept == url) {
            (self.images).push_back((url.clone(), None, picture.image.clone()));
        }
    }
}

/// How big the image at `url` turned out to be, when it was fetched this run.
fn known_size(url: &SharedString) -> Option<(f32, f32)> {
    KEPT.lock().sizes.get(url).copied()
}

/// Count `url`'s picture as drawn in this frame, and see to a pass after it.
fn keep(url: &SharedString, picture: &Picture, cx: &mut App) {
    let mut kept = KEPT.lock();
    let frame = Some(kept.frame);
    kept.images.retain(|(kept, ..)| kept != url);
    (kept.images).push_back((url.clone(), frame, picture.image.clone()));
    if !std::mem::replace(&mut kept.pass, true) {
        cx.defer(|cx| let_go(cx, true));
    }
}

/// Let go of the images drawn longest ago, and those that landed and weren't drawn, while
/// they keep more than [`KEPT_BYTES`]: gpui keeps every asset it loads, and an image on the
/// GPU, until told. What the latest frame that drew images drew stays however much it is,
/// as it's on screen; `after_frame` is a pass right after that frame.
fn let_go(cx: &mut App, after_frame: bool) {
    let gone = {
        let mut kept = KEPT.lock();
        let drawn = match after_frame {
            true => kept.frame,
            false => kept.frame.saturating_sub(1),
        };
        if after_frame {
            (kept.frame, kept.pass) = (drawn + 1, false);
        }
        let bytes = |image: &RenderImage| {
            let size = image.size(0);
            (size.width.0 * size.height.0 * 4) as usize
        };
        let total: usize = kept.images.iter().map(|(_, _, image)| bytes(image)).sum();
        let mut over = total.saturating_sub(KEPT_BYTES);
        let mut gone = Vec::new();
        kept.images.retain(|(url, frame, image)| {
            if over == 0 || *frame >= Some(drawn) {
                return true;
            }
            over = over.saturating_sub(bytes(image));
            gone.push((url.clone(), image.clone()));
            false
        });
        gone
    };
    for (url, image) in gone {
        cx.remove_asset::<Fetched>(&url);
        cx.drop_image(image, None);
    }
}
