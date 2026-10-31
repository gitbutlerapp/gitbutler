//! How runs end. A run leaves a file saying it's running, which quitting takes away, so the next
//! launch knows the last one didn't quit: a panic says so in a file of its own, and on macOS the
//! system's crash report says what else stopped it. Both go to error tracking then, as a panic
//! on the UI thread ends the app before what's queued can be sent, and a crash runs no code of
//! ours at all.

use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::{Value, json};

use super::{Kind, Metrics, Props, queue, redact};

/// The most frames an exception keeps, nearest where it happened, as PostHog's SDK keeps.
const MAX_FRAMES: usize = 64;
/// The running process's ID. When it was written is when that run started.
const RUNNING_FILE: &str = "gpui-running";
/// A panic that ended the app, as the `$exception` the next launch sends. When it was written
/// is when it happened.
const PANIC_FILE: &str = "gpui-panic.json";

/// Report each panic, then do what was done before. One on the UI thread ends the app, so it's
/// kept for the next launch; anywhere else it fails one task, and it goes out now.
pub(super) fn catch_panics() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("unnamed");
        let props = panic_props(info, name);
        if name == "main" {
            if let Some(file) = file(PANIC_FILE)
                && let Ok(json) = serde_json::to_vec(&props)
            {
                std::fs::write(file, json).ok();
            }
        } else if let Some(location) = info.location()
            && super::limit(format!("panic {}:{}", location.file(), location.line())).is_some()
        {
            queue(Kind::Errors, "$exception", props);
        }
        previous(info);
    }));
}

/// Report what ended the last run and mark this one running: `clean` when it quit, or there
/// was none, `panic` or `crash` when it says which, and `unclean` when nothing does, as after a
/// force quit, a power cut, or a crash the system kept no report of.
pub(super) fn previous_run(metrics: &Metrics) -> &'static str {
    let last = file(RUNNING_FILE).and_then(|file| {
        let started = std::fs::metadata(&file).ok()?.modified().ok()?;
        let pid: u32 = std::fs::read_to_string(file).ok()?.trim().parse().ok()?;
        Some((pid, started))
    });
    let panicked = file(PANIC_FILE).and_then(|file| {
        let at = std::fs::metadata(&file).ok()?.modified().ok()?;
        let json = std::fs::read(&file).ok()?;
        std::fs::remove_file(file).ok();
        Some((at, serde_json::from_slice::<Props>(&json).ok()?))
    });
    mark_running();
    let crashed = match (last, &panicked) {
        (Some((pid, started)), None) => crash_report(pid, started),
        _ => None,
    };
    let exit = match (last, &panicked, &crashed) {
        (_, Some(_), _) => "panic",
        (_, _, Some(_)) => "crash",
        (Some(_), None, None) => "unclean",
        (None, None, None) => "clean",
    };
    for (at, props) in panicked.into_iter().chain(crashed) {
        metrics.send(Kind::Errors, "$exception", props, Some(at.into()));
    }
    exit
}

/// Quitting: this run ended as it should.
pub(super) fn ended_cleanly() {
    if let Some(file) = file(RUNNING_FILE) {
        std::fs::remove_file(file).ok();
    }
}

/// `path` from its crate's folder on, as `but-gpui/src/state/account.rs`: the folders before
/// it are the build machine's.
pub(super) fn short_path(path: &str) -> String {
    let parts: Vec<&str> = path.split(['/', '\\']).collect();
    let from =
        (parts.iter().rposition(|part| *part == "src")).map_or(0, |src| src.saturating_sub(1));
    parts.get(from..).unwrap_or_default().join("/")
}

/// `name` in the build's channel's data folder, which the desktop app keeps its own in.
fn file(name: &str) -> Option<PathBuf> {
    but_path::app_data_dir().ok().map(|dir| dir.join(name))
}

fn mark_running() {
    let Some(file) = file(RUNNING_FILE) else {
        return;
    };
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    std::fs::write(file, std::process::id().to_string()).ok();
}

/// The panic `info`, on the thread `thread`, for error tracking: what it said and where, and
/// the stack that led there.
fn panic_props(info: &PanicHookInfo<'_>, thread: &str) -> Props {
    let payload = info.payload();
    let message = (payload.downcast_ref::<&str>().copied())
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("Box<dyn Any>");
    let at = info.location().map_or_else(String::new, |location| {
        format!(" at {}:{}", short_path(location.file()), location.line())
    });
    let mut frames = Vec::new();
    for frame in backtrace::Backtrace::new().frames() {
        for symbol in frame.symbols() {
            let function = symbol.name().map(|name| format!("{name:#}"));
            frames.push(frame_json(
                function.as_deref(),
                symbol.filename().map(Path::to_string_lossy).as_deref(),
                symbol.lineno(),
            ));
        }
    }
    // Innermost first: this hook and the panic's own machinery go, down to where it panicked.
    // Further out, a thread's start catches panics with the same machinery, which stays.
    let machinery = |frame: &Value| {
        (frame.get("function").and_then(Value::as_str)).is_some_and(|function| {
            [
                "panicking",
                "begin_panic",
                "rust_begin_unwind",
                "rust_end_short_backtrace",
            ]
            .iter()
            .any(|part| function.contains(part))
        })
    };
    if let Some(first) = frames.iter().position(machinery) {
        let after = (frames.iter().skip(first)).position(|frame| !machinery(frame));
        frames.drain(..after.map_or(frames.len(), |after| first + after));
    }
    frames.truncate(MAX_FRAMES);
    frames.reverse();
    let level = match thread {
        "main" => "fatal",
        _ => "error",
    };
    exception(
        "Panic",
        format!("{}{at}", redact(message, &Default::default())),
        "panic",
        frames,
        json!({ "$exception_level": level, "thread": thread }),
    )
}

/// What macOS reported about the crash of the process `pid`, if it wrote a report since
/// `started`, with when. It writes the report up to ten seconds after the crash, later than a
/// quick relaunch, so this looks again for a while; quitting doesn't wait for it.
#[cfg(target_os = "macos")]
fn crash_report(pid: u32, started: SystemTime) -> Option<(SystemTime, Props)> {
    let reports = but_path::home_dir()?.join("Library/Logs/DiagnosticReports");
    let exe = std::env::current_exe().ok()?;
    let exe = exe.file_name()?.to_str()?;
    (0..6).find_map(|attempt| {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
        std::fs::read_dir(&reports)
            .ok()?
            .flatten()
            .find_map(|entry| {
                let name = entry.file_name();
                let name = name.to_str()?;
                if !(name.starts_with(&format!("{exe}-")) && name.ends_with(".ips")) {
                    return None;
                }
                let written = entry.metadata().ok()?.modified().ok()?;
                if written < started {
                    return None;
                }
                let ips = std::fs::read_to_string(entry.path()).ok()?;
                Some((written, report::parse(&ips, pid)?))
            })
    })
}

#[cfg(not(target_os = "macos"))]
fn crash_report(_: u32, _: SystemTime) -> Option<(SystemTime, Props)> {
    None
}

/// Reading macOS's crash reports.
#[cfg(any(target_os = "macos", test))]
mod report {
    use std::collections::HashMap;

    use serde::Deserialize;
    use serde_json::{Value, json};

    use super::{MAX_FRAMES, Props, exception, frame_json, redact};

    /// A crash report as macOS writes it (`.ips`): a line of JSON about the report, then
    /// the report's own JSON, of which this reads only what says where it crashed.
    #[derive(Deserialize)]
    struct Report {
        pid: u32,
        exception: Option<ReportException>,
        /// Messages the system's libraries left, as `abort() called`.
        #[serde(default)]
        asi: HashMap<String, Vec<String>>,
        #[serde(default)]
        threads: Vec<ReportThread>,
    }

    #[derive(Deserialize)]
    struct ReportException {
        #[serde(rename = "type")]
        kind: String,
        signal: Option<String>,
    }

    #[derive(Deserialize)]
    struct ReportThread {
        #[serde(default)]
        triggered: bool,
        name: Option<String>,
        queue: Option<String>,
        #[serde(default)]
        frames: Vec<ReportFrame>,
    }

    #[derive(Deserialize)]
    struct ReportFrame {
        symbol: Option<String>,
    }

    /// The crash in `ips`, a macOS crash report, if it's the process `pid`'s: what stopped
    /// it, and the stack of the thread it stopped.
    pub(super) fn parse(ips: &str, pid: u32) -> Option<Props> {
        let (_, body) = ips.split_once('\n')?;
        let report: Report = serde_json::from_str(body).ok()?;
        if report.pid != pid {
            return None;
        }
        let thread = report.threads.iter().find(|thread| thread.triggered)?;
        let mut frames: Vec<Value> = (thread.frames.iter().take(MAX_FRAMES))
            .map(|frame| {
                let function = (frame.symbol.as_deref())
                    .map(|symbol| format!("{:#}", backtrace::SymbolName::new(symbol.as_bytes())));
                frame_json(function.as_deref(), None, None)
            })
            .collect();
        frames.reverse();
        let (kind, signal) = report
            .exception
            .map_or((String::from("Crash"), None), |e| (e.kind, e.signal));
        let said = report
            .asi
            .values()
            .flatten()
            .next()
            .map(|said| redact(said, &Default::default()));
        let value = [signal, said]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(": ");
        let thread = (thread.name.as_deref()).or(thread.queue.as_deref());
        Some(exception(
            &kind,
            value,
            "crash",
            frames,
            json!({ "$exception_level": "fatal", "thread": thread }),
        ))
    }
}

/// A stack frame as PostHog's SDK sends one it resolved itself, which PostHog shows as it is.
fn frame_json(function: Option<&str>, file: Option<&str>, line: Option<u32>) -> Value {
    let function = function.unwrap_or("?");
    let file = file.map(short_path);
    // GitButler's own crates, which PostHog shows first. Debug info names an inlined function
    // without its path, as `{async_block#0}`, but its file says whose it is.
    let unqualified = function.trim_start_matches('<');
    let in_app = ["but_", "gitbutler_"]
        .iter()
        .any(|ours| unqualified.starts_with(ours))
        || file.as_deref().is_some_and(|file| {
            ["but-", "gitbutler-"]
                .iter()
                .any(|ours| file.starts_with(ours))
        });
    json!({
        "function": function,
        "filename": file,
        "lineno": line,
        "lang": "rust",
        "platform": "native",
        "in_app": in_app,
        "synthetic": false,
        "client_resolved": true,
    })
}

/// The properties of a `$exception` event of `kind` saying `value`, with `frames` outermost
/// first, and `more`.
fn exception(kind: &str, value: String, mechanism: &str, frames: Vec<Value>, more: Value) -> Props {
    let mut props = Props::new();
    props.insert(
        "$exception_list".into(),
        json!([{
            "type": kind,
            "value": value,
            "mechanism": { "type": mechanism, "handled": false },
            "stacktrace": { "type": "raw", "frames": frames },
        }]),
    );
    if let Value::Object(more) = more {
        props.extend(more);
    }
    props
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape of a report macOS wrote for a crash in the client, cut down.
    const REPORT: &str = r#"{"app_name":"gitbutler-gpui","bug_type":"309"}
{"pid":42,"procName":"gitbutler-gpui",
 "exception":{"type":"EXC_BREAKPOINT","signal":"SIGTRAP"},
 "asi":{"libsystem_malloc.dylib":["BUG IN CLIENT OF LIBMALLOC: memory corruption of free block"]},
 "threads":[{"frames":[]},{"triggered":true,"name":"gix_status::index_as_worktree","frames":[
   {"imageOffset":100,"imageIndex":1,"symbol":"_xzm_xzone_malloc_freelist_outlined"},
   {"imageOffset":16,"imageIndex":0,"symbol":"_RNvNtCs1_8but_gpui7backend3run"}]}],
 "usedImages":[{"name":"gitbutler-gpui"},{"name":"libsystem_malloc.dylib"}]}"#;

    #[test]
    fn a_crash_report_becomes_the_crashed_threads_exception() {
        assert!(
            report::parse(REPORT, 7).is_none(),
            "another process's report isn't the last run's"
        );
        let props = report::parse(REPORT, 42).expect("it's pid 42's");
        let exception = &props["$exception_list"][0];
        assert_eq!(exception["type"], "EXC_BREAKPOINT");
        assert_eq!(
            exception["value"],
            "SIGTRAP: BUG IN CLIENT OF LIBMALLOC: memory corruption of free block"
        );
        let frames = exception["stacktrace"]["frames"]
            .as_array()
            .expect("frames");
        assert_eq!(
            frames[0]["function"], "but_gpui::backend::run",
            "outermost first, demangled"
        );
        assert_eq!(frames[0]["in_app"], true, "the client's own code");
        assert_eq!(frames[1]["in_app"], false, "the system's isn't");
        assert_eq!(props["thread"], "gix_status::index_as_worktree");
    }

    #[test]
    fn paths_keep_their_crate_and_lose_the_build_machines_folders() {
        assert_eq!(
            short_path("/Users/runner/work/gitbutler/crates/but-gpui/src/state/account.rs"),
            "but-gpui/src/state/account.rs"
        );
        assert_eq!(
            short_path(r"C:\Users\me\.cargo\registry\src\index\gix-0.80.0\src\lib.rs"),
            "gix-0.80.0/src/lib.rs",
            "as Windows writes them"
        );
    }
}
