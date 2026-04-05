//! Embedded frontend assets for `--features embedded-frontend`.
//!
//! Build the frontend first:
//!   pnpm --filter @gitbutler/desktop build
//!
//! Then run with the feature enabled:
//!   cargo run -p but-server --features embedded-frontend

use axum::{body::Body, http::StatusCode, response::Response};
use rust_embed::RustEmbed;

/// Returns `'sha256-<base64>'` hashes for every inline `<script>` block in
/// the embedded `index.html`. Used to populate `script-src` in the CSP without
/// resorting to `'unsafe-inline'`.
pub fn inline_script_hashes() -> Vec<String> {
    let html = match Assets::get("index.html") {
        Some(f) => f.data,
        None => return vec![],
    };
    let html = match std::str::from_utf8(&html) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    extract_inline_script_hashes(html)
}

fn extract_inline_script_hashes(html: &str) -> Vec<String> {
    let mut hashes = Vec::new();
    let mut pos = 0;
    while let Some(tag_start) = html[pos..].find("<script") {
        let abs = pos + tag_start + "<script".len();
        // Find end of opening tag
        let Some(tag_close) = html[abs..].find('>') else {
            break;
        };
        let attrs = &html[abs..abs + tag_close];
        let body_start = abs + tag_close + 1;
        let Some(close) = html[body_start..].find("</script>") else {
            break;
        };
        // Only hash inline scripts — skip those with a src attribute
        if !attrs.contains("src=") {
            let body = &html[body_start..body_start + close];
            hashes.push(crate::sha256_csp_hash(body.as_bytes()));
        }
        pos = body_start + close + "</script>".len();
    }
    hashes
}

/// Forces recompilation when the frontend build changes so rust-embed
/// picks up new assets. Value is set by build.rs.
const _: &str = env!("EMBEDDED_FRONTEND_HASH");

#[derive(RustEmbed)]
#[folder = "../../apps/desktop/build/"]
struct Assets;

/// Axum fallback handler — serves embedded static files with SPA fallback.
///
/// Any path not matched by but-server's API routes is handled here:
/// - Known files are served with the correct Content-Type.
/// - Unknown paths fall back to `index.html` for client-side routing.
/// - `index.html` has a `<meta name="gitbutler-api-url">` tag injected so the
///   frontend can use the correct API URL at runtime instead of the build-time
///   default baked into the bundle.
pub async fn serve(uri: axum::http::Uri, api_url: String) -> Response<Body> {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if path == "index.html" {
        serve_index(&api_url)
    } else {
        serve_path(path, &api_url)
    }
}

/// Serves `index.html` with a `<meta name="gitbutler-api-url">` tag injected
/// into `<head>` so the frontend can discover the server's API URL at runtime.
fn serve_index(api_url: &str) -> Response<Body> {
    let file = match Assets::get("index.html") {
        Some(f) => f,
        None => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(axum::http::header::CONTENT_TYPE, "text/plain")
                .body(Body::from("index.html not found in embedded assets"))
                .unwrap();
        }
    };
    let html = match std::str::from_utf8(&file.data) {
        Ok(s) => s,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(axum::http::header::CONTENT_TYPE, "text/plain")
                .body(Body::from("index.html is not valid UTF-8"))
                .unwrap();
        }
    };
    let meta = format!(
        r#"<meta name="gitbutler-api-url" content="{}">"#,
        crate::html_escape(api_url)
    );
    let injected = if let Some(pos) = html.find("</head>") {
        format!(
            "{}{}</head>{}",
            &html[..pos],
            meta,
            &html[pos + "</head>".len()..]
        )
    } else {
        // No </head> found — prepend the meta tag.
        format!("{meta}{html}")
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(injected))
        .unwrap()
}

fn serve_path(path: &str, api_url: &str) -> Response<Body> {
    match Assets::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path)
                .first_or_octet_stream()
                .to_string();
            Response::builder()
                .status(StatusCode::OK)
                .header(axum::http::header::CONTENT_TYPE, mime)
                .body(Body::from(file.data))
                .unwrap()
        }
        // SPA fallback — serve index.html so the client-side router can handle it.
        None => serve_index(api_url),
    }
}
