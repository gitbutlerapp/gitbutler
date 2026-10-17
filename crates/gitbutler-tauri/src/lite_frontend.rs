//! The Lite frontend is embedded under `lite/` beside the Svelte one; serving it means looking every
//! asset up there instead, so Tauri's `index.html` fallback lands on Lite's.

use std::borrow::Cow;

use tauri::{
    Assets, Context, Runtime,
    utils::assets::{AssetKey, AssetsIter, CspHash},
};

const PREFIX: &str = "/lite";

struct LiteFrontend<R: Runtime>(Box<dyn Assets<R>>);

/// Holds the context's place while its assets move into [`LiteFrontend`].
struct NoAssets;

impl<R: Runtime> Assets<R> for NoAssets {
    fn get(&self, _key: &AssetKey) -> Option<Cow<'_, [u8]>> {
        None
    }

    fn iter(&self) -> Box<AssetsIter<'_>> {
        Box::new(std::iter::empty())
    }

    fn csp_hashes(&self, _html_path: &AssetKey) -> Box<dyn Iterator<Item = CspHash<'_>> + '_> {
        Box::new(std::iter::empty())
    }
}

fn lite_key(key: &AssetKey) -> AssetKey {
    AssetKey::from(format!("{PREFIX}{}", key.as_ref()))
}

impl<R: Runtime> Assets<R> for LiteFrontend<R> {
    fn get(&self, key: &AssetKey) -> Option<Cow<'_, [u8]>> {
        self.0.get(&lite_key(key))
    }

    fn iter(&self) -> Box<AssetsIter<'_>> {
        Box::new(self.0.iter().filter_map(|(path, bytes)| {
            let path = path.strip_prefix(PREFIX)?.to_owned();
            Some((Cow::Owned(path), bytes))
        }))
    }

    fn csp_hashes(&self, html_path: &AssetKey) -> Box<dyn Iterator<Item = CspHash<'_>> + '_> {
        self.0.csp_hashes(&lite_key(html_path))
    }
}

fn embeds_lite<R: Runtime>(context: &Context<R>) -> bool {
    context
        .assets()
        .get(&lite_key(&"index.html".into()))
        .is_some()
}

/// Make `context` serve the Lite frontend in place of the Svelte one, unless the build has none.
pub fn serve<R: Runtime>(context: &mut Context<R>) -> bool {
    if tauri::is_dev() {
        context.config_mut().build.dev_url =
            Some("http://localhost:1421".parse().expect("valid Lite dev URL"));
    } else if !embeds_lite(context) {
        return false;
    }
    let embedded = context.set_assets(Box::new(NoAssets));
    context.set_assets(Box::new(LiteFrontend(embedded)));
    true
}
