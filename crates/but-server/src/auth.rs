//! Authentication for remote access to but-server.
//!
//! When the server is configured with a remote origin (via `--tunnel` or
//! `--remote-origin`), all non-localhost requests must be authenticated.
//! Authentication is performed by validating the user's GitButler access
//! token against the GitButler API and checking that the authenticated
//! user matches the local owner.

use std::{sync::Arc, time::Duration};

use axum::{
    body::Body,
    extract::State,
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use tokio::sync::RwLock;

/// How long to cache a validated token before re-checking with the API.
const TOKEN_CACHE_TTL: Duration = Duration::from_secs(300);

const COOKIE_NAME: &str = "butler_token";

/// Partial user response from the GitButler API `/api/login/whoami` endpoint.
#[derive(Debug, Deserialize)]
struct ApiUser {
    id: u64,
}

/// Response from `POST /api/login/token`.
#[derive(Debug, Deserialize)]
struct LoginTokenResponse {
    /// The full URL to redirect the user's browser to for login.
    url: String,
}

/// A cached token validation result.
struct CachedValidation {
    token: String,
    user_id: u64,
    validated_at: std::time::Instant,
}

/// Shared authentication state.
pub struct AuthState {
    owner_id: u64,
    http: reqwest::Client,
    cache: RwLock<Option<CachedValidation>>,
    /// Base path prefix for auth routes, e.g. "/api" when `--base-path=/api`.
    base_path: String,
    /// GitButler API base URL, e.g. "https://app.gitbutler.com".
    api_url: String,
}

impl AuthState {
    /// Create a new auth state for the given local owner user ID, base path, and API URL.
    pub fn new(owner_id: u64, base_path: impl Into<String>, api_url: impl Into<String>) -> Self {
        Self {
            owner_id,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .connect_timeout(Duration::from_secs(5))
                .build()
                .expect("failed to build reqwest client"),
            cache: RwLock::new(None),
            base_path: base_path.into(),
            api_url: api_url.into(),
        }
    }

    /// Validate an access token against the GitButler API.
    /// Returns `true` if the token belongs to the local owner.
    pub async fn validate_token(&self, token: &str) -> Result<bool, reqwest::Error> {
        // Check cache first.
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.as_ref()
                && cached.token == token
                && cached.validated_at.elapsed() < TOKEN_CACHE_TTL
            {
                return Ok(cached.user_id == self.owner_id);
            }
        }

        // Call the GitButler API.
        let url = format!("{}/api/login/whoami", self.api_url);
        let resp = self
            .http
            .get(&url)
            .header("X-Auth-Token", token)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(false);
        }

        let user: ApiUser = resp.json().await?;

        // Update cache.
        {
            let mut cache = self.cache.write().await;
            *cache = Some(CachedValidation {
                token: token.to_string(),
                user_id: user.id,
                validated_at: std::time::Instant::now(),
            });
        }

        Ok(user.id == self.owner_id)
    }

    /// Get the gitbutler.com login URL for the user to visit.
    async fn login_url(&self) -> anyhow::Result<String> {
        let url = format!("{}/api/login/token", self.api_url);
        let resp: LoginTokenResponse = self
            .http
            .post(&url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(resp.url)
    }
}

/// Extract the auth token from the request (cookie or header).
fn extract_token(req: &axum::extract::Request<Body>) -> Option<String> {
    // Check X-Auth-Token header first.
    if let Some(val) = req.headers().get("X-Auth-Token") {
        return val.to_str().ok().map(|s| s.to_string());
    }

    // Check cookie.
    if let Some(cookie_header) = req.headers().get(header::COOKIE)
        && let Ok(cookies) = cookie_header.to_str()
    {
        for cookie in cookies.split(';') {
            let cookie = cookie.trim();
            if let Some(value) = cookie.strip_prefix(&format!("{COOKIE_NAME}=")) {
                return Some(percent_decode_cookie_value(value));
            }
        }
    }

    None
}

/// Middleware that enforces authentication for remote requests.
///
/// Skips auth for:
/// - Localhost connections (already handled by `localhost_only_middleware`)
/// - Requests to `/auth/*` routes (or `<base_path>/auth/*` when a base path is set)
/// - When remote access is not configured
pub async fn auth_middleware(
    State(auth): State<Option<Arc<AuthState>>>,
    req: axum::extract::Request<Body>,
    next: Next,
) -> Response {
    // Bypass all authentication when explicitly opted in.
    if crate::allow_anyone() {
        return next.run(req).await;
    }

    let auth = match auth {
        Some(auth) => auth,
        // Remote access not enabled — skip auth.
        None => return next.run(req).await,
    };

    // Skip auth for /auth/* routes (accounting for an optional base path prefix,
    // e.g. /api/auth/ when --tunnel sets api_base to /api).
    let path = req.uri().path();
    if path.starts_with("/auth/")
        || (!auth.base_path.is_empty() && path.starts_with(&format!("{}/auth/", auth.base_path)))
    {
        return next.run(req).await;
    }

    // Check for a valid token.
    match extract_token(&req) {
        Some(token) => match auth.validate_token(&token).await {
            Ok(true) => next.run(req).await,
            Ok(false) => {
                tracing::warn!("Remote auth: token valid but user is not the owner");
                StatusCode::FORBIDDEN.into_response()
            }
            Err(e) => {
                tracing::error!("Remote auth: failed to validate token: {e}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        },
        None => {
            // No token — redirect browsers to login, return 401 for API calls.
            if accepts_html(&req) {
                let login_path = format!("{}/auth/login", auth.base_path);
                Redirect::temporary(&login_path).into_response()
            } else {
                StatusCode::UNAUTHORIZED.into_response()
            }
        }
    }
}

fn accepts_html(req: &axum::extract::Request<Body>) -> bool {
    req.headers()
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains("text/html"))
}

// --- Auth route handlers ---

/// `GET /auth/login` — Serves the login page.
///
/// Opens gitbutler.com in a new tab and shows a field to paste the access
/// token that gitbutler.com displays after login.
pub async fn login(State(auth): State<Option<Arc<AuthState>>>) -> Response {
    let auth = match auth {
        Some(auth) => auth,
        None => return StatusCode::NOT_FOUND.into_response(),
    };

    let login_url = match auth.login_url().await {
        Ok(url) => url,
        Err(e) => {
            tracing::error!("Failed to get login URL: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let base_path = &auth.base_path;
    // The script body is kept in a separate function so its SHA-256 hash can
    // be computed once at startup and included in the Content-Security-Policy.
    // Dynamic data (login_url) is passed via a <meta> tag instead of being
    // inlined in the script, keeping the script content stable across requests.
    let script = login_page_script(base_path);
    let login_url_escaped = crate::html_escape(&login_url);
    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>GitButler | Log in</title>
  <meta name="gb-login-url" content="{login_url_escaped}">
  <style>
    :root {{
      --bg-1: #fff;
      --bg-2: #f3f2f1;
      --border-2: #cac6c3;
      --text-1: #1c1917;
      --text-2: #7c716a;
      --text-danger: #dc2626;
      --fill-pop: #25b1b1;
      --fill-pop-hover: #1d8a8a;
      --radius-s: 0.25rem;
      --radius-xl: 20px;
      --font: "Inter", ui-sans-serif, system-ui, -apple-system, sans-serif;
      --font-accent: "But Head", Georgia, serif;
      --transition-fast: 0.05s ease;
    }}
    @media (prefers-color-scheme: dark) {{
      :root {{
        --bg-1: #272321;
        --bg-2: #1c1917;
        --border-2: #4f4844;
        --text-1: #f3f2f1;
        --text-2: #aaa29d;
        --fill-pop: #1d8a8a;
        --fill-pop-hover: #25b1b1;
      }}
    }}
    *, *::before, *::after {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{
      display: flex;
      align-items: center;
      justify-content: center;
      min-height: 100dvh;
      padding-bottom: env(safe-area-inset-bottom);
      background: var(--bg-2);
      font-family: var(--font);
      color: var(--text-1);
    }}
    .service-form {{
      display: flex;
      flex-direction: column;
      width: 100%;
      max-width: 540px;
      padding: 50px 60px 40px;
      border-radius: var(--radius-xl);
      background: var(--bg-1);
    }}
    h1 {{
      font-family: var(--font-accent);
      font-size: 2.625rem;
      font-weight: 400;
      line-height: 110%;
      margin-bottom: 16px;
    }}
    .warning {{
      font-size: 12px;
      line-height: 1.5;
      color: #a46204;
      background: #fef7ee;
      border: 1px solid #f0bc73;
      border-radius: var(--radius-s);
      padding: 8px 12px;
      margin-bottom: 16px;
    }}
    @media (prefers-color-scheme: dark) {{
      .warning {{ color: #f0bc73; background: #a46204; border-color: #f0bc73; }}
    }}
    .content {{
      display: flex;
      flex-direction: column;
      gap: 12px;
    }}
    p {{ font-size: 13px; line-height: 1.5; color: var(--text-2); }}
    a {{ color: var(--text-1); text-decoration: underline; transition: color var(--transition-fast); }}
    a:hover {{ color: var(--text-2); }}
    input {{
      width: 100%;
      padding: 7px 10px;
      border: 1px solid var(--border-2);
      border-radius: var(--radius-s);
      background: var(--bg-1);
      color: var(--text-1);
      font-family: var(--font);
      font-size: 13px;
      outline: none;
      transition: border-color var(--transition-fast);
      margin-top: 8px;
    }}
    input::placeholder {{ color: var(--text-2); }}
    input:hover, input:focus {{ border-color: var(--text-2); }}
    .actions {{
      display: flex;
      gap: 8px;
      margin-top: 8px;
    }}
    button {{
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding: 6px 12px;
      border: 1px solid var(--border-2);
      border-radius: var(--radius-s);
      background: transparent;
      color: var(--text-1);
      font-family: var(--font);
      font-size: 12px;
      font-weight: 500;
      cursor: pointer;
      transition: background var(--transition-fast), border-color var(--transition-fast);
    }}
    button:hover {{ background: var(--bg-2); }}
    button.primary {{
      background: var(--fill-pop);
      border-color: var(--fill-pop);
      color: #fff;
    }}
    button.primary:hover {{ background: var(--fill-pop-hover); border-color: var(--fill-pop-hover); }}
    button:disabled {{ opacity: 0.5; cursor: not-allowed; }}
    @keyframes spin {{ to {{ transform: rotate(360deg); }} }}
    .spinner {{
      display: none;
      width: 12px; height: 12px;
      border: 1.5px solid currentColor;
      border-top-color: transparent;
      border-radius: 50%;
      animation: spin 0.6s linear infinite;
    }}
    button.loading .btn-label {{ display: none; }}
    button.loading .spinner {{ display: block; }}
    .error {{
      font-size: 12px;
      color: var(--text-danger);
      display: none;
    }}
    .footer {{
      display: flex;
      justify-content: space-between;
      margin-top: 40px;
      font-size: 12px;
      color: var(--text-2);
    }}
    .footer a {{ color: var(--text-2); }}
    .footer a:hover {{ color: var(--text-1); }}
    @media (max-width: 600px) {{
      .service-form {{ padding: 30px 20px 20px; border-radius: 0; }}
      .footer {{ flex-direction: column; gap: 8px; margin-top: 24px; }}
    }}
  </style>
</head>
<body>
  <div class="service-form">
    <h1>Log in to GitButler</h1>
    <div class="warning">
      ⚠ This feature is a work in progress, please exercise caution when exposing
      your repository over a tunnel!
    </div>
    <div class="content">
      <p>
        <a href="{login_url_escaped}" target="_blank" rel="noopener noreferrer" id="loginLink">Open gitbutler.com to sign in</a>
        — then paste the access token shown on that page below.
      </p>
      <div>
        <input id="token" type="password" placeholder="Paste your access token here" />
        <p class="error" id="error">Invalid token or wrong account. Please try again.</p>
      </div>
      <div class="actions">
        <button class="primary" id="submitBtn"><span class="btn-label">Submit</span><span class="spinner"></span></button>
      </div>
    </div>
    <div class="footer">
      <p></p>
      <p>Need help? <a href="https://docs.gitbutler.com" target="_blank" rel="noopener noreferrer">docs.gitbutler.com</a></p>
    </div>
  </div>
  <script>{script}</script>
</body>
</html>"#
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html")
        .body(Body::from(html))
        .unwrap()
}

#[derive(Deserialize)]
pub struct TokenSubmission {
    token: String,
}

/// `POST /auth/callback` — Validates a pasted access token and sets the session cookie.
pub async fn callback(
    State(auth): State<Option<Arc<AuthState>>>,
    axum::Json(body): axum::Json<TokenSubmission>,
) -> Response {
    let auth = match auth {
        Some(auth) => auth,
        None => return StatusCode::NOT_FOUND.into_response(),
    };

    match auth.validate_token(&body.token).await {
        Ok(true) => {
            let remote_origin = crate::allowed_remote_origin().unwrap_or_default();
            let secure = remote_origin.starts_with("https://");
            let encoded_token = percent_encode_cookie_value(&body.token);
            let mut attrs = vec![
                format!("{COOKIE_NAME}={encoded_token}"),
                "HttpOnly".to_string(),
                "SameSite=Lax".to_string(),
                "Path=/".to_string(),
            ];
            if secure {
                attrs.push("Secure".to_string());
            }
            let cookie = attrs.join("; ");
            Response::builder()
                .status(StatusCode::OK)
                .header(header::SET_COOKIE, cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"ok":true}"#))
                .unwrap()
        }
        Ok(false) => Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"ok":false,"error":"not the owner"}"#))
            .unwrap(),
        Err(e) => {
            tracing::error!("Token validation failed: {e}");
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"ok":false,"error":"validation error"}"#))
                .unwrap()
        }
    }
}

/// `GET /auth/logout` — Clears the auth cookie.
pub async fn logout() -> Response {
    let cookie = format!("{COOKIE_NAME}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0");
    Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, "/")
        .header(header::SET_COOKIE, cookie)
        .body(Body::empty())
        .unwrap()
}

/// Decodes a percent-encoded cookie value back to its original string.
fn percent_decode_cookie_value(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (
                char::from(bytes[i + 1]).to_digit(16),
                char::from(bytes[i + 2]).to_digit(16),
            )
        {
            out.push(((hi << 4) | lo) as u8);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| value.to_string())
}

/// Percent-encodes characters that are not allowed in cookie values per RFC 6265.
///
/// Cookie values must not contain whitespace, double-quotes, commas, semicolons,
/// or backslashes. GitButler tokens are typically base64url/JWT-like and rarely
/// need encoding, but we encode defensively.
fn percent_encode_cookie_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        // cookie-octet = %x21 / %x23-2B / %x2D-3A / %x3C-5B / %x5D-7E
        // Everything else gets percent-encoded.
        if matches!(byte, 0x21 | 0x23..=0x2B | 0x2D..=0x3A | 0x3C..=0x5B | 0x5D..=0x7E) {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(
                char::from_digit((byte >> 4) as u32, 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
            out.push(
                char::from_digit((byte & 0xF) as u32, 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
        }
    }
    out
}

/// Returns the static JavaScript body for the login page.
///
/// The login URL is intentionally NOT embedded here — it is passed via a
/// `<meta name="gb-login-url">` tag so that the script content stays stable
/// across requests and its SHA-256 hash can be pre-computed for the CSP.
///
/// Only `base_path` is baked in; it is fixed at server startup.
pub fn login_page_script(base_path: &str) -> String {
    format!(
        r#"
    const loginUrl = document.querySelector('meta[name="gb-login-url"]').getAttribute('content');
    document.getElementById('loginLink').href = loginUrl;
    async function submit() {{
      const token = document.getElementById('token').value.trim();
      if (!token) return;
      const btn = document.getElementById('submitBtn');
      btn.disabled = true;
      btn.classList.add('loading');
      try {{
        const r = await fetch('{base_path}/auth/callback', {{
          method: 'POST',
          headers: {{ 'Content-Type': 'application/json' }},
          body: JSON.stringify({{ token }}),
          credentials: 'include',
        }});
        const data = await r.json();
        if (data.ok) {{
          window.location.href = '/';
        }} else {{
          document.getElementById('error').style.display = 'block';
        }}
      }} catch (_) {{
        document.getElementById('error').style.display = 'block';
      }} finally {{
        btn.disabled = false;
        btn.classList.remove('loading');
      }}
    }}
    document.getElementById('submitBtn').addEventListener('click', submit);
    document.getElementById('token').addEventListener('keydown', e => {{
      if (e.key === 'Enter') submit();
    }});"#
    )
}
