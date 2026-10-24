//! Text for times, counts, paths and errors, as every surface words them.

use gpui::SharedString;

/// Seconds since the epoch.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

/// Whether `seconds` (since the epoch) is within two minutes of now: not much later either,
/// as a file a clock that's off dated years ahead.
pub fn just_now(seconds: i64) -> bool {
    (now() - seconds).abs() < 120
}

/// A commit's title as shown, with a stand-in when its message is empty.
pub fn commit_title(title: &str) -> SharedString {
    match title {
        "" => "No message".into(),
        title => title.to_owned().into(),
    }
}

/// "1 commit", "3 commits".
pub fn plural(n: usize, word: &str) -> String {
    match n {
        1 => format!("1 {word}"),
        // As English spells it: branches, not branchs.
        n if ["s", "x", "ch", "sh"].iter().any(|end| word.ends_with(end)) => {
            format!("{n} {word}es")
        }
        n => format!("{n} {word}s"),
    }
}

/// `text` quoted in a menu item, cut to 40 characters.
pub fn quoted(text: &str) -> String {
    match text.chars().count() > 40 {
        true => format!("\"{}…\"", text.chars().take(39).collect::<String>()),
        false => format!("\"{text}\""),
    }
}

/// `path` as shown, with the home folder as `~`, so one cut short keeps more of its end.
pub fn short_path(path: &std::path::Path) -> String {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    match home.and_then(|home| {
        path.strip_prefix(home)
            .ok()
            .map(|rest| rest.display().to_string())
    }) {
        Some(rest) => format!("~/{rest}"),
        None => path.display().to_string(),
    }
}

/// What to show of an error: Git's own complaints when it ran Git, like "fatal: Could not
/// read from remote repository." and why ssh couldn't get there, said just before it, and
/// what the remote said back, as a server's hook does when it refuses a push, as the rest
/// repeats the command it ran; else its first lines, without the codes libgit2 appends
/// ("; class=Index (10); code=Locked (-14)").
pub fn error_summary(text: &str) -> String {
    let lines: Vec<&str> = (text.lines())
        .map(|line| line.split("; class=").next().unwrap_or(line).trim())
        .collect();
    let git = |line: &str| line.starts_with("fatal:") || line.starts_with("error:");
    let mut said: Vec<&str> = Vec::new();
    let mut keep = |line| {
        // Git's line again with a wrapper's ": …" after it keeps its short form.
        let wraps = |long: &str, short: &str| {
            (long.strip_prefix(short)).is_some_and(|rest| rest.is_empty() || rest.starts_with(':'))
        };
        match (said.iter_mut()).find(|said| wraps(said, line) || wraps(line, said)) {
            Some(said) if line.len() < said.len() => *said = line,
            Some(_) => {}
            None => said.push(line),
        }
    };
    for (ix, &line) in lines.iter().enumerate() {
        let (remote, line) = match line.strip_prefix("remote:") {
            Some(said) => (true, said.trim()),
            None => (false, line),
        };
        if line.is_empty() || !(remote || git(line)) {
            continue;
        }
        let before = ix.checked_sub(1).and_then(|ix| lines.get(ix)).copied();
        if line == "fatal: Could not read from remote repository."
            // Not the label a wrapper puts over Git's output, when ssh said nothing.
            && let Some(cause) =
                before.filter(|before| !before.is_empty() && !git(before) && !before.ends_with(':'))
        {
            keep(cause);
        }
        keep(line);
    }
    match said.is_empty() {
        true => lines.get(..4).unwrap_or(&lines).join("\n"),
        false => said.join("\n"),
    }
}

/// A span of `seconds`: "42s", "3m 5s", "1h 12m".
pub fn duration(seconds: i64) -> String {
    match seconds {
        ..60 => format!("{seconds}s"),
        60..3600 => format!("{}m {}s", seconds / 60, seconds % 60),
        _ => format!("{}h {}m", seconds / 3600, seconds % 3600 / 60),
    }
}

/// `seconds` (since the epoch) as how long ago: "just now", "5m ago".
pub fn ago(seconds: i64) -> String {
    match relative_time(seconds).as_str() {
        "now" => "just now".into(),
        ago => format!("{ago} ago"),
    }
}

pub fn relative_time(seconds: i64) -> String {
    let delta = (now() - seconds).max(0);
    match delta {
        0..60 => "now".into(),
        60..3600 => format!("{}m", delta / 60),
        3600..86_400 => format!("{}h", delta / 3600),
        86_400..2_592_000 => format!("{}d", delta / 86_400),
        2_592_000..31_536_000 => format!("{}mo", delta / 2_592_000),
        _ => format!("{}y", delta / 31_536_000),
    }
}

/// `seconds` (since the epoch) as the time here, the way but.dev dates a commit:
/// "16/10/2026, 21:04".
pub fn local_time(seconds: i64) -> String {
    local(seconds, "%d/%m/%Y, %H:%M")
}

/// `seconds` (since the epoch) in `format`, at the time here then: a summer moment keeps
/// summer time in winter.
fn local(seconds: i64, format: &'static str) -> String {
    use chrono::TimeZone as _;
    match chrono::Local.timestamp_opt(seconds, 0).single() {
        Some(at) => at.format(format).to_string(),
        None => seconds.to_string(),
    }
}

/// The time of day `seconds` (since the epoch) fall on here: "14:15".
pub fn local_clock(seconds: i64) -> String {
    local(seconds, "%H:%M")
}

/// The day `seconds` (since the epoch) fall on here, as a short date: "21 Oct".
pub fn local_date(seconds: i64) -> String {
    local(seconds, "%-d %b")
}

/// The same with its weekday: "Wed, 21 Oct".
pub fn local_weekday(seconds: i64) -> String {
    local(seconds, "%a, %-d %b")
}

/// The day `seconds` (since the epoch) fell on here, to tell one day's from another's.
pub fn local_day(seconds: i64) -> String {
    local(seconds, "%Y-%m-%d")
}

/// The day `seconds` fell on, as Tauri's history heads a day's operations: Today, Yesterday,
/// or the weekday and date, with the year when it isn't this one.
pub fn day_label(seconds: i64) -> String {
    let today = now();
    match local_day(seconds) {
        day if day == local_day(today) => "Today".into(),
        day if day == local_day(today - 86_400) => "Yesterday".into(),
        _ if local(seconds, "%Y") == local(today, "%Y") => local_weekday(seconds),
        _ => local(seconds, "%a, %-d %b %Y"),
    }
}

#[cfg(test)]
mod tests {
    use super::error_summary;

    #[test]
    fn a_git_failure_shows_what_git_said_once() {
        let failed = "fetching remote `origin` failed: git command exited with code 128:\n\n\
            ARGS:\n[\"fetch\"]\n\nSTDERR:\nfatal: '/x.git' does not appear to be a git repository\n\
            fatal: Could not read from remote repository.\n: git command exited with code 128:\n\n\
            STDERR:\nfatal: '/x.git' does not appear to be a git repository\n";
        assert_eq!(
            error_summary(failed),
            "fatal: '/x.git' does not appear to be a git repository\n\
             fatal: Could not read from remote repository.",
            "Git's own lines, each once"
        );
        let rejected = "error: remote unpack failed\n ! [remote rejected] a -> b\n\
            error: failed to push some refs to '/u.git': git command exited with code 1:\n\n\
            STDERR:\nerror: remote unpack failed\nerror: failed to push some refs to '/u.git'\n";
        assert_eq!(
            error_summary(rejected),
            "error: remote unpack failed\nerror: failed to push some refs to '/u.git'",
            "a line repeated with the wrapper's words after it, in its short form"
        );
        let declined = "git command exited with non-zero exit code 1:\nSTDERR:\n\
            remote: pushes are frozen for the release        \n\
            ! [remote rejected] a -> b (pre-receive hook declined)\n\
            error: failed to push some refs to '/u.git'\n";
        assert_eq!(
            error_summary(declined),
            "pushes are frozen for the release\nerror: failed to push some refs to '/u.git'",
            "the remote's reason, as its hook gave it"
        );
        let denied = "fetching remote `origin` failed: git command exited with code 128:\n\n\
            STDERR:\ngit@github.com: Permission denied (publickey).\n\
            fatal: Could not read from remote repository.\n\n\
            Please make sure you have the correct access rights\nand the repository exists.\n";
        assert_eq!(
            error_summary(denied),
            "git@github.com: Permission denied (publickey).\n\
             fatal: Could not read from remote repository.",
            "why ssh couldn't get in, which Git gives up after"
        );
        let silent = "git command exited with code 128:\n\nSTDERR:\n\
            fatal: Could not read from remote repository.\n";
        assert_eq!(
            error_summary(silent),
            "fatal: Could not read from remote repository.",
            "not the wrapper's label over Git's output, when ssh said nothing"
        );
        assert_eq!(
            error_summary("the index is locked; class=Index (10); code=Locked (-14)"),
            "the index is locked",
            "without libgit2's codes"
        );
        assert_eq!(
            error_summary("Branch not found\nmore\nand more\nstill\ncut"),
            "Branch not found\nmore\nand more\nstill",
            "any other error by its first lines"
        );
    }
}
