//! Cloudflare tunnel support.
//!
//! Two modes are supported:
//!
//! * **Quick tunnel** (`start`) — no account needed. `cloudflared` is spawned
//!   with `--url` and assigns a random `trycloudflare.com` URL.  The URL is
//!   parsed from cloudflared's stderr output and returned.
//!
//! * **Named tunnel** (`start_named`) — requires a pre-configured tunnel.
//!   `cloudflared` is invoked with `--hostname` and the URL is the supplied
//!   hostname (known upfront, no parsing needed).  See
//!   <https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/> for setup.
//!
//! Requires `cloudflared` to be installed. If it is not found a clear
//! error message with install instructions is printed.

use colored::Colorize as _;
use tokio::io::AsyncBufReadExt as _;

const TUNNEL_URL_TIMEOUT_SECS: u64 = 30;

/// Spawn `cloudflared` with the given arguments, returning the child process.
///
/// Translates a "not found" OS error into a user-friendly message with install
/// instructions; other spawn errors are returned as-is.
fn spawn_cloudflared(args: &[&str]) -> anyhow::Result<tokio::process::Child> {
    tokio::process::Command::new("cloudflared")
        .args(args)
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                let install_hint = if cfg!(target_os = "macos") {
                    "brew install cloudflared"
                } else if cfg!(target_os = "windows") {
                    "winget install --id Cloudflare.cloudflared"
                } else {
                    "https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/"
                };
                anyhow::anyhow!(
                    "{}\nInstall: {}",
                    "cloudflared is not installed.".bold(),
                    install_hint.cyan()
                )
            } else {
                anyhow::anyhow!("Failed to spawn cloudflared: {e}")
            }
        })
}

/// Spawn two tasks that each read one stream and forward lines to a shared channel.
///
/// Using `chain` would read stdout to EOF before touching stderr; this
/// interleaves both so neither stream starves the other.
fn merge_output(child: &mut tokio::process::Child) -> tokio::sync::mpsc::Receiver<String> {
    let (tx, rx) = tokio::sync::mpsc::channel(64);

    let stdout = child.stdout.take().expect("stdout is piped");
    let tx_out = tx.clone();
    tokio::spawn(async move {
        let mut lines = tokio::io::BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if tx_out.send(line).await.is_err() {
                break;
            }
        }
    });

    let stderr = child.stderr.take().expect("stderr is piped");
    tokio::spawn(async move {
        let mut lines = tokio::io::BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if tx.send(line).await.is_err() {
                break;
            }
        }
    });

    rx
}

/// Spawn a cloudflared quick-tunnel pointed at `http://127.0.0.1:{port}`.
///
/// Waits up to 30 seconds for cloudflared to report the tunnel URL, then
/// returns the URL and the child process. The child **must** be kept alive
/// for the tunnel to remain open; dropping it kills the process.
pub async fn start(port: u16) -> anyhow::Result<(String, tokio::process::Child)> {
    let mut child = spawn_cloudflared(&[
        "tunnel",
        "--url",
        &format!("http://127.0.0.1:{port}"),
        "--no-autoupdate",
    ])?;

    // cloudflared may write to stdout or stderr — merge both into one channel.
    let mut rx = merge_output(&mut child);

    let url = tokio::time::timeout(
        std::time::Duration::from_secs(TUNNEL_URL_TIMEOUT_SECS),
        async {
            while let Some(line) = rx.recv().await {
                if let Some(url) = extract_url(&line) {
                    return Ok(url);
                }
            }
            Err(anyhow::anyhow!(
                "cloudflared exited before reporting a tunnel URL"
            ))
        },
    )
    .await
    .map_err(|_| {
        anyhow::anyhow!(
            "Timed out after {TUNNEL_URL_TIMEOUT_SECS}s waiting for cloudflared tunnel URL"
        )
    })??;

    // Keep draining cloudflared's output so its stdio pipes never fill up.
    tokio::spawn(async move {
        while let Some(line) = rx.recv().await {
            eprintln!("{line}");
        }
    });

    Ok((url, child))
}

/// Spawn a named cloudflared tunnel pointed at `http://127.0.0.1:{port}`.
///
/// Uses `cloudflared tunnel run --url ... <name>` which connects the
/// pre-configured named tunnel to the local server.  The caller must have
/// already run:
///
/// ```text
/// cloudflared tunnel login
/// cloudflared tunnel create <name>
/// cloudflared tunnel route dns <name> <hostname>
/// ```
///
/// `name` is the tunnel name or UUID (e.g. `mytunnel`).
/// `hostname` is the public hostname you routed to the tunnel (e.g.
/// `but.example.com`) — used only as the display URL and CORS origin.
///
/// Returns the canonical `https://<hostname>` URL and the child process.
/// The child **must** be kept alive for the tunnel to remain open.
pub async fn start_named(
    name: &str,
    hostname: &str,
    port: u16,
) -> anyhow::Result<(String, tokio::process::Child)> {
    let url_arg = format!("http://127.0.0.1:{port}");
    let mut child = spawn_cloudflared(&["tunnel", "run", "--url", &url_arg, name])?;

    // cloudflared may write to stdout or stderr — merge both into one channel.
    let mut rx = merge_output(&mut child);

    // Wait until cloudflared logs that at least one connection is registered.
    // Forward every line to stderr so the user can see what cloudflared is doing,
    // and collect lines so we can surface them if the process exits unexpectedly.
    tokio::time::timeout(
        std::time::Duration::from_secs(TUNNEL_URL_TIMEOUT_SECS),
        async {
            let mut seen = Vec::new();
            while let Some(line) = rx.recv().await {
                eprintln!("{line}");
                if is_connected(&line) {
                    return Ok(());
                }
                seen.push(line);
            }
            // Process exited — include whatever it printed so the user can diagnose.
            let detail = if seen.is_empty() {
                "(no output)".to_string()
            } else {
                seen.join("\n")
            };
            Err(anyhow::anyhow!(
                "cloudflared exited before the tunnel was established:\n{detail}"
            ))
        },
    )
    .await
    .map_err(|_| {
        anyhow::anyhow!(
            "Timed out after {TUNNEL_URL_TIMEOUT_SECS}s waiting for cloudflared to connect"
        )
    })??;

    // Keep draining cloudflared's output so its stdio pipes never fill up.
    // Without this, the write-blocking cloudflared would stall mid-proxy.
    tokio::spawn(async move {
        while let Some(line) = rx.recv().await {
            eprintln!("{line}");
        }
    });

    // Strip any scheme prefix so we always produce `https://<bare-host>`.
    let bare = hostname
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .split('/') // reject any path component
        .next()
        .unwrap_or(hostname);
    let url = format!("https://{bare}");
    Ok((url, child))
}

/// Returns `true` when a cloudflared log line confirms the tunnel is connected.
fn is_connected(line: &str) -> bool {
    // cloudflared emits this once the first connection to the Cloudflare edge
    // is registered and ready to accept traffic.
    line.contains("Registered tunnel connection")
        || line.contains("Connection registered")
        || line.contains("connsReady=1")
}

/// Extract a `https://*.trycloudflare.com` URL from a cloudflared log line.
fn extract_url(line: &str) -> Option<String> {
    let marker = ".trycloudflare.com";
    let end = line.find(marker)? + marker.len();
    let start = line[..end].rfind("https://")?;
    Some(line[start..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tunnel_url_from_log_line() {
        let line = "2024-01-01T00:00:00Z INF |  https://example-words-here.trycloudflare.com  |";
        assert_eq!(
            extract_url(line),
            Some("https://example-words-here.trycloudflare.com".into())
        );
    }

    #[test]
    fn returns_none_for_unrelated_lines() {
        assert_eq!(extract_url("INFO starting tunnel"), None);
    }

    #[test]
    fn detects_registered_connection() {
        assert!(is_connected(
            "2024-01-01T00:00:00Z INF Registered tunnel connection connIndex=0"
        ));
        assert!(is_connected("INF connsReady=1"));
        assert!(!is_connected("INF Starting tunnel tunnelID=abc"));
    }
}
