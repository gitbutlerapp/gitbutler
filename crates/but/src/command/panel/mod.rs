//! `but panel`: a live, read-only view of the workspace, served to the browser from localhost.
//!
//! The page and its script are compiled in. Its data comes from the same `but-api` functions the
//! GUI uses: the detailed workspace graph, the worktree changes, commit details and file diffs.
//! Requests are handled one at a time on the calling thread, which keeps the single `Context` free
//! of concurrent access.

use std::{
    fmt::Write as _,
    io::{self, BufReader, Read as _, Write as _},
    net::{TcpListener, TcpStream},
    time::Duration,
};

use anyhow::Context as _;
use but_api::{
    commit::json::ChangesSource,
    diff::{ComputeLineStats, json::CommitDetails},
};
use but_ctx::Context;
use serde_json::json;

use crate::{CliResult, args::panel::Platform, bad_input, utils::IntermediateChannel};

const INDEX_HTML: &str = include_str!("index.html");
const APP_JS: &str = include_str!("app.js");

/// Requests are a request line and a few headers; anything larger is not from the page.
const MAX_REQUEST_BYTES: u64 = 16 * 1024;

pub fn serve(ctx: &mut Context, mut out: IntermediateChannel<'_>, args: Platform) -> CliResult<()> {
    let Platform { port, no_open } = args;

    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(listener) => listener,
        Err(err) if err.kind() == io::ErrorKind::AddrInUse => {
            return Err(bad_input(format!("Port {port} is already in use"))
                .hint("Pass `--port` to serve the panel on another port")
                .into());
        }
        Err(err) => {
            return Err(anyhow::Error::from(err)
                .context(format!("Could not listen on 127.0.0.1:{port}"))
                .into());
        }
    };

    let workdir = ctx.workdir_or_fail()?;
    let repo_name = workdir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| workdir.display().to_string());

    let url = format!("http://localhost:{port}");
    writeln!(
        out,
        "Serving the {repo_name} panel at {url} (Ctrl-C to stop)"
    )?;
    if !no_open && let Err(err) = but_api::open::open_url(url.clone()) {
        writeln!(out, "Could not open a browser ({err:#}); visit {url}")?;
    }

    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        if let Err(err) = handle_connection(ctx, &repo_name, port, stream) {
            // A dropped or malformed connection only affects that one request.
            tracing::debug!(?err, "panel request failed");
        }
    }
    Ok(())
}

fn handle_connection(
    ctx: &mut Context,
    repo_name: &str,
    port: u16,
    stream: TcpStream,
) -> anyhow::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let request = read_request(BufReader::new((&stream).take(MAX_REQUEST_BYTES)))?;
    let response = match route(&request, port) {
        Route::Index => Response::Page {
            content_type: "text/html; charset=utf-8",
            body: INDEX_HTML,
        },
        Route::Script => Response::Page {
            content_type: "text/javascript; charset=utf-8",
            body: APP_JS,
        },
        Route::Workspace => data_response(
            workspace_json(ctx)
                .map(|data| json!({ "repo": repo_name, "workspace": data.0, "changes": data.1 })),
        ),
        Route::Commit { id } => data_response(commit_json(ctx, &id)),
        Route::Diff { path, commit } => data_response(diff_json(ctx, &path, commit.as_deref())),
        Route::Error { status } => Response::Error { status },
    };
    write_response(&stream, response)
}

/// The workspace graph and the main worktree's uncommitted changes.
fn workspace_json(ctx: &mut Context) -> anyhow::Result<(serde_json::Value, serde_json::Value)> {
    let mut guard = ctx.exclusive_worktree_access();
    // This command keeps one context for its whole run; drop the cached workspace so each request
    // sees changes made by other processes since the last one.
    ctx.invalidate_workspace(guard.write_permission());
    let workspace = but_api::workspace::get_workspace(ctx, guard.read_permission())?;
    let changes = but_api::diff::changes_in_worktree_with_perm(
        ctx,
        ChangesSource::Head,
        false,
        guard.read_permission(),
    )?;
    Ok((
        serde_json::to_value(workspace)?,
        serde_json::to_value(changes.worktree_changes.changes)?,
    ))
}

/// A commit's metadata, changed files and line statistics.
fn commit_json(ctx: &Context, id: &str) -> anyhow::Result<serde_json::Value> {
    let commit_id = parse_commit_id(id)?;
    let details: CommitDetails =
        but_api::diff::commit_details_with_line_stats(ctx, commit_id)?.into();
    Ok(serde_json::to_value(details)?)
}

/// The patch of one file, changed in `commit` or, without one, in the uncommitted changes.
fn diff_json(ctx: &Context, path: &str, commit: Option<&str>) -> anyhow::Result<serde_json::Value> {
    let change = match commit {
        Some(id) => but_api::diff::commit_details(ctx, parse_commit_id(id)?, ComputeLineStats::No)?
            .diff_with_first_parent
            .into_iter()
            .find(|change| change.path == path.as_bytes())
            .map(Into::into),
        None => but_api::diff::changes_in_worktree(ctx, ChangesSource::Head, false)?
            .worktree_changes
            .changes
            .into_iter()
            .find(|change| change.path_bytes == path.as_bytes()),
    }
    .with_context(|| format!("'{path}' has no change to show"))?;
    let patch = but_api::diff::tree_change_diffs(ctx, change)?;
    Ok(serde_json::to_value(patch)?)
}

fn parse_commit_id(id: &str) -> anyhow::Result<gix::ObjectId> {
    id.parse()
        .with_context(|| format!("Invalid commit ID: {id}"))
}

fn data_response(result: anyhow::Result<serde_json::Value>) -> Response {
    Response::Json(match result {
        Ok(data) => json!({ "ok": true, "data": data }),
        Err(err) => json!({ "ok": false, "error": format!("{err:#}") }),
    })
}

#[derive(Debug, PartialEq)]
struct Request {
    method: String,
    target: String,
    host: Option<String>,
}

fn read_request(mut reader: impl io::BufRead) -> anyhow::Result<Request> {
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let (Some(method), Some(target)) = (parts.next(), parts.next()) else {
        anyhow::bail!("malformed request line: {request_line:?}");
    };

    let mut host = None;
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let header = line.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("host")
        {
            host = Some(value.trim().to_owned());
        }
    }

    Ok(Request {
        method: method.to_owned(),
        target: target.to_owned(),
        host,
    })
}

enum Response {
    Page {
        content_type: &'static str,
        body: &'static str,
    },
    Json(serde_json::Value),
    Error {
        status: &'static str,
    },
}

#[derive(Debug, PartialEq)]
enum Route {
    Index,
    Script,
    Workspace,
    Commit {
        id: String,
    },
    /// One file's patch, in `commit` or in the uncommitted changes.
    Diff {
        path: String,
        commit: Option<String>,
    },
    Error {
        status: &'static str,
    },
}

fn route(request: &Request, port: u16) -> Route {
    // Only the page itself may call in. Checking `Host` stops another site from reaching this
    // server by pointing its own domain at 127.0.0.1 (DNS rebinding).
    let host_is_local = request.host.as_deref().is_some_and(|host| {
        host == format!("localhost:{port}") || host == format!("127.0.0.1:{port}")
    });
    if !host_is_local {
        return Route::Error {
            status: "403 Forbidden",
        };
    }
    if request.method != "GET" {
        return Route::Error {
            status: "405 Method Not Allowed",
        };
    }

    let (path, query) = request
        .target
        .split_once('?')
        .unwrap_or((request.target.as_str(), ""));
    let Some(params) = query_params(query) else {
        return Route::Error {
            status: "400 Bad Request",
        };
    };
    let param = |name: &str| {
        params
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
            .filter(|value| !value.is_empty())
    };

    match path {
        "/" => Route::Index,
        "/app.js" => Route::Script,
        "/api/workspace" => Route::Workspace,
        "/api/commit" => match param("id") {
            Some(id) => Route::Commit { id },
            None => Route::Error {
                status: "400 Bad Request",
            },
        },
        "/api/diff" => match param("path") {
            Some(path) => Route::Diff {
                path,
                commit: param("commit"),
            },
            None => Route::Error {
                status: "400 Bad Request",
            },
        },
        _ => Route::Error {
            status: "404 Not Found",
        },
    }
}

/// Split and decode `a=1&b=2`, or `None` if an escape is malformed.
fn query_params(query: &str) -> Option<Vec<(String, String)>> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            Some((percent_decode(key)?, percent_decode(value)?))
        })
        .collect()
}

/// Decode the `%XX` escapes `encodeURIComponent` produces, or `None` if they are malformed.
fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = input.get(i + 1..i + 3)?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn write_response(mut stream: &TcpStream, response: Response) -> anyhow::Result<()> {
    let (status, content_type, body) = match response {
        Response::Page { content_type, body } => ("200 OK", content_type, body.to_owned()),
        Response::Json(value) => (
            "200 OK",
            "application/json",
            serde_json::to_string(&value).context("serializing the panel response")?,
        ),
        Response::Error { status } => (status, "text/plain; charset=utf-8", status.to_owned()),
    };
    write!(
        stream,
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {len}\r\n\
         Cache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Connection: close\r\n\r\n\
         {body}",
        len = body.len(),
    )?;
    stream.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get(target: &str, host: Option<&str>) -> Request {
        Request {
            method: "GET".to_owned(),
            target: target.to_owned(),
            host: host.map(str::to_owned),
        }
    }

    const LOCAL: Option<&str> = Some("localhost:7789");

    #[test]
    fn reads_the_request_line_and_host_header() {
        let raw = "GET /api/workspace HTTP/1.1\r\nhost: localhost:7789\r\nAccept: */*\r\n\r\n";
        let request = read_request(raw.as_bytes()).expect("a well-formed request parses");
        assert_eq!(
            request,
            get("/api/workspace", LOCAL),
            "the header name matches case-insensitively"
        );
    }

    #[test]
    fn routes_the_page_and_its_data() {
        assert_eq!(route(&get("/", LOCAL), 7789), Route::Index);
        assert_eq!(route(&get("/app.js", LOCAL), 7789), Route::Script);
        assert_eq!(route(&get("/api/workspace", LOCAL), 7789), Route::Workspace);
        assert_eq!(
            route(&get("/api/commit?id=abc123", LOCAL), 7789),
            Route::Commit {
                id: "abc123".into()
            }
        );
        assert_eq!(
            route(&get("/api/diff?path=src%2Fa%20b.rs", LOCAL), 7789),
            Route::Diff {
                path: "src/a b.rs".into(),
                commit: None
            },
            "a path without a commit asks for the uncommitted change, decoded"
        );
        assert_eq!(
            route(&get("/api/diff?path=a.rs&commit=abc123", LOCAL), 7789),
            Route::Diff {
                path: "a.rs".into(),
                commit: Some("abc123".into())
            },
        );
    }

    #[test]
    fn rejects_requests_the_page_would_not_make() {
        let forbidden = Route::Error {
            status: "403 Forbidden",
        };
        let bad = Route::Error {
            status: "400 Bad Request",
        };
        assert_eq!(
            route(&get("/", Some("evil.example:7789")), 7789),
            forbidden,
            "a rebound foreign host is refused"
        );
        assert_eq!(
            route(&get("/", Some("localhost:1234")), 7789),
            forbidden,
            "the host must name this server's port"
        );
        assert_eq!(
            route(&get("/", None), 7789),
            forbidden,
            "a host is required"
        );
        assert_eq!(
            route(
                &Request {
                    method: "POST".into(),
                    ..get("/api/workspace", LOCAL)
                },
                7789
            ),
            Route::Error {
                status: "405 Method Not Allowed"
            },
            "the panel is read-only"
        );
        assert_eq!(
            route(&get("/api/diff?path=%zz", LOCAL), 7789),
            bad,
            "malformed escapes are not guessed at"
        );
        assert_eq!(
            route(&get("/api/commit", LOCAL), 7789),
            bad,
            "a commit needs an id"
        );
        assert_eq!(
            route(&get("/nope", LOCAL), 7789),
            Route::Error {
                status: "404 Not Found"
            },
            "only the page, its script and the data routes exist"
        );
    }
}
