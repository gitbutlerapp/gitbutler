use anyhow::Result;
use but_settings::AppSettingsWithDiskSync;
use tauri::utils::config::{Csp, CspDirectiveSources};

// NOTE: The base CSP in `tauri.conf.json` includes `'wasm-unsafe-eval'` in `script-src`.
// This is required by Shiki's Oniguruma WASM regex engine for syntax highlighting.
// Unlike `'unsafe-eval'`, `'wasm-unsafe-eval'` only permits WebAssembly compilation —
// it does not allow `eval()`, `new Function()`, or other dynamic JS execution.

/// Constructs a new CSP object with additional `connect-src` and `img-src` hosts as provided by the AppSettings.
pub fn csp_with_extras(
    csp: Option<Csp>,
    settings: &AppSettingsWithDiskSync,
) -> Result<Option<Csp>> {
    let extra_csp = settings.get()?.clone().extra_csp;

    let hosts = extra_csp
        .hosts
        .iter()
        .filter_map(|host| url::Url::parse(host).ok())
        .map(|h| h.to_string())
        .collect::<Vec<_>>();

    let img_src = extra_csp
        .img_src
        .iter()
        .filter_map(|host| url::Url::parse(host).ok())
        .map(|h| h.to_string())
        .collect::<Vec<_>>();

    if hosts.is_empty() && img_src.is_empty() {
        return Ok(csp); // noop
    }

    let new_csp = if let Some(Csp::DirectiveMap(mut map)) = csp {
        if !hosts.is_empty()
            && let Some(CspDirectiveSources::Inline(sources)) = map.get_mut("connect-src")
        {
            sources.push_str(&format!(" {}", hosts.join(" ")));
        }
        if !img_src.is_empty()
            && let Some(CspDirectiveSources::Inline(sources)) = map.get_mut("img-src")
        {
            sources.push_str(&format!(" {}", img_src.join(" ")));
        }
        Some(Csp::DirectiveMap(map))
    } else {
        None
    };

    Ok(new_csp)
}
