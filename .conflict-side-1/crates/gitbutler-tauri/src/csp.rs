use anyhow::Result;
use but_settings::AppSettingsWithDiskSync;
use tauri::utils::config::{Csp, CspDirectiveSources};

/// Constructs a new CSP object with additional `connect-src` hosts as provided by the AppSettings.
pub fn csp_with_extras(
    csp: Option<Csp>,
    settings: &AppSettingsWithDiskSync,
) -> Result<Option<Csp>> {
    let hosts = settings
        .get()?
        .clone()
        .extra_csp
        .hosts
        .iter()
        .filter_map(|host| url::Url::parse(host).ok())
        .map(|h| h.to_string())
        .collect::<Vec<_>>();

    if hosts.is_empty() {
        return Ok(csp); // noop
    }

    let new_csp = if let Some(Csp::DirectiveMap(mut map)) = csp {
        if let Some(CspDirectiveSources::Inline(sources)) = map.get_mut("connect-src") {
            sources.push_str(&format!(" {}", hosts.join(" ")));
        }
        Some(Csp::DirectiveMap(map))
    } else {
        None
    };

    Ok(new_csp)
}
