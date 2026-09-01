//! Temporary handling of syntax retired by the July 2026 command revamp.
//!
//! The revamp changed `commit`, `squash`, `move`, `amend`, `uncommit` and
//! `discard`, and removed `rub`. The retired forms are widespread in LLM
//! training data, so agents keep producing them. Two mitigations live here:
//!
//! - `but commit <branch> -c -m "message" --changes <id>,<id>` is rewritten
//!   into the modern `but commit -b <branch> -m "message" <change>...`
//!   equivalent, the command runs normally, and a hint teaching the new
//!   syntax is printed at the end.
//! - For the other revamped commands, a rejected command line that looks like
//!   retired syntax gets a teaching hint before the parse error — with the
//!   concrete modern equivalent whenever it can be derived mechanically.
//!   Nothing is translated or executed for these.
//!
//! The retired grammars are redeclared below as clap structs, so command
//! lines are tokenized exactly as the pre-revamp binary did (short-flag
//! bundles, `=`-attached values, comma-separated `--changes`). For `commit`,
//! translation then maps typed fields to modern argv. Everything only runs as
//! a fallback after the modern parser has rejected the command line, so
//! current syntax is never affected.
//! Translation is refused (surfacing the hint followed by the original parse
//! error) for retired flags with no modern equivalent, and whenever the
//! rewrite could widen the commit's scope beyond what was asked, such as an
//! empty `--changes` value.
//!
//! One deliberate deviation from the retired parser: the modern `--branch`
//! flag creates the branch when it does not exist, while the retired
//! positional required an existing branch unless `-c` was given. A retired
//! invocation naming a nonexistent branch without `-c` therefore creates it
//! instead of erroring; completing the requested commit is preferred over
//! faithfully reproducing the old error.
//!
//! Delete this module and its call sites in `lib.rs` once the
//! `retiredCommitSyntax` and `retiredSyntaxHint` command props show the old
//! syntax has aged out of common use. (`rub` attempts are visible without a
//! dedicated prop, as external-command events with `externalSubcommand:
//! "rub"`.) The resolve-time suggestions — commit's `-b` hint and squash's
//! `-t` hint — are permanent teaching improvements outside this module and
//! are not part of these criteria.

use std::ffi::OsString;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{
    args::{find_subcommand, metrics::CommandName},
    theme::Paint as _,
};

static USED: AtomicBool = AtomicBool::new(false);

/// Record that a retired command line was translated and executed.
pub(crate) fn mark_used() {
    USED.store(true, Ordering::Relaxed);
}

/// Whether the current invocation used retired syntax.
pub(crate) fn was_used() -> bool {
    USED.load(Ordering::Relaxed)
}

/// The hint teaching the modern syntax, printed to stderr.
pub(crate) fn hint(agent: bool) -> String {
    note(
        "this invocation used retired `but commit` syntax. The modern form is:\n\n    \
         but commit -b <branch> -m \"message\" <change>...\n\n\
         See `but commit --help` for details.",
        agent,
    )
}

/// The hint for an invocation of a command removed by the revamp, which
/// surfaces as an unknown external subcommand rather than a parse failure.
pub(crate) fn removed_command_hint(command_name: &std::ffi::OsStr, agent: bool) -> Option<String> {
    (command_name == "rub").then(|| {
        note(
            "`but rub` was retired. Squashing sources into a target is now:\n\n    \
             but squash <source>... -t <target>\n\n\
             Moving sources is `but move <source>...` with a placement flag \
             (`--below`/`--above` a commit, `--branch <branch>`).\n\
             See `but squash --help` and `but move --help` for details.",
            agent,
        )
    })
}

/// A stderr note in the shared teaching-hint format, with a pointer to the
/// bundled skill docs when an agent is driving the CLI.
fn note(body: &str, agent: bool) -> String {
    let t = crate::theme::get();
    let mut text = format!("\n{} {body}\n", t.attention.paint("note:"));
    if agent {
        text.push_str(
            "Run `but skill install` (or `but skill check --update`) to load current command docs.\n",
        );
    }
    text
}

/// The outcome of attempting to translate a command line.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Translation {
    /// No retired-only token was found; this is not a retired invocation.
    NotRetired,
    /// Retired syntax was identified, but translating it safely is not
    /// possible (e.g. it could widen the commit's scope). The original parse
    /// error should stand, prefixed with the teaching hint.
    Refused,
    /// The modern equivalent command line.
    Translated(Vec<OsString>),
}

/// Global options valid in both grammars; they may legally follow the
/// subcommand, so every retired grammar must accept them.
#[derive(Debug, clap::Args)]
struct RetiredGlobals {
    #[clap(long)]
    json: bool,
    #[clap(long)]
    status_after: bool,
}

/// The retired `but commit` argument grammar. Only `message`, `branch`,
/// `create`, `all` and `changes` have modern equivalents; the other flags are
/// declared so that invocations using them are recognized as retired syntax
/// and refused with the teaching hint.
#[derive(Debug, clap::Parser)]
struct RetiredCommit {
    #[clap(short, long)]
    message: Option<String>,
    #[clap(long, value_name = "FILE")]
    message_file: Option<std::path::PathBuf>,
    /// The branch to commit onto; the retired grammar's only positional.
    branch: Option<String>,
    #[clap(short, long)]
    create: bool,
    #[clap(long)]
    before: Option<String>,
    #[clap(long)]
    after: Option<String>,
    /// Documented in the retired grammar as a no-op compatibility flag for
    /// `git commit -a`; committing everything is the modern default too.
    #[clap(short, long)]
    all: bool,
    #[clap(short = 'n', long, alias = "no-verify")]
    no_hooks: bool,
    #[clap(short = 'i', long, require_equals = true)]
    ai: Option<Option<String>>,
    #[clap(short = 'p', long, value_delimiter = ',')]
    changes: Vec<String>,
    #[clap(long)]
    diff: bool,
    #[clap(long)]
    no_diff: bool,
    #[clap(flatten)]
    globals: RetiredGlobals,
}

/// Rewrite a retired `but commit` command line into the modern equivalent.
///
/// Returns [`Translation::NotRetired`] unless the tail after the `commit`
/// subcommand parses under the retired grammar and contains at least one
/// retired-only token.
pub(crate) fn translate_commit(args: &[OsString]) -> Translation {
    let Some((commit_ix, subcommand)) = find_subcommand(args) else {
        return Translation::NotRetired;
    };
    if subcommand != "commit" {
        return Translation::NotRetired;
    }
    let (prefix, tail) = args.split_at(commit_ix + 1);
    let parse = std::iter::once(OsString::from("but commit")).chain(tail.iter().cloned());
    let Ok(retired) = <RetiredCommit as clap::Parser>::try_parse_from(parse) else {
        // Not even the retired parser accepts this; the modern error stands.
        return Translation::NotRetired;
    };
    let RetiredCommit {
        message,
        message_file,
        branch,
        create,
        before,
        after,
        all,
        no_hooks,
        ai,
        changes,
        diff,
        no_diff,
        globals: RetiredGlobals { json, status_after },
    } = retired;

    let unsupported = message_file.is_some()
        || before.is_some()
        || after.is_some()
        || no_hooks
        || ai.is_some()
        || diff
        || no_diff;
    let changes_given = !changes.is_empty();
    if !(create || all || changes_given || unsupported) {
        return Translation::NotRetired;
    }

    // Refuse change lists the retired binary would not have committed as-is:
    // it rejected empty components (`--changes one,`) during ID resolution,
    // so dropping them here could silently commit less than was asked (or,
    // for a fully empty list, everything uncommitted). Hyphen-leading
    // components would be re-parsed as flags when re-emitted as positionals.
    let unsafe_changes = changes
        .iter()
        .any(|change| change.is_empty() || change.starts_with('-'));
    if unsupported || unsafe_changes {
        return Translation::Refused;
    }

    let mut translated = prefix.to_vec();
    if let Some(message) = message {
        // The `=`-attached form also binds hyphen-leading messages.
        translated.push(format!("--message={message}").into());
    }
    if json {
        translated.push("--json".into());
    }
    if status_after {
        translated.push("--status-after".into());
    }
    if let Some(branch) = &branch {
        // The `=`-attached form binds unambiguously to the optional-value flag.
        translated.push(format!("--branch={branch}").into());
    }
    translated.extend(changes.into_iter().map(OsString::from));
    if branch.is_none() && create {
        // Retired `-c` without a branch name created a generated-name branch;
        // a bare trailing `--branch` does the same in the modern grammar.
        translated.push("--branch".into());
    }
    Translation::Translated(translated)
}

/// A teaching hint for a command line the modern parser rejected, when the
/// failure looks like retired syntax for one of the revamped subcommands.
/// Returns the command's metrics name alongside the hint so the caller can
/// record that retired syntax is still being produced.
///
/// Unlike [`translate_commit`], nothing is executed on the caller's behalf:
/// the hint names the concrete modern equivalent when it can be derived
/// mechanically, and otherwise just points at the command's help.
pub(crate) fn parse_failure_hint(args: &[OsString], agent: bool) -> Option<(CommandName, String)> {
    let (ix, subcommand) = find_subcommand(args)?;
    let tail = &args[ix + 1..];
    let (command, hint) = match subcommand.to_str()? {
        "amend" => (CommandName::Amend, amend_hint(tail, agent)),
        "move" => (CommandName::Move, move_hint(tail, agent)),
        "squash" => (CommandName::Squash, squash_hint(tail, agent)),
        "uncommit" => (CommandName::Uncommit, uncommit_hint(tail, agent)),
        _ => return None,
    };
    Some((command, hint?))
}

/// Whether a failure hint embeds a concrete modern command line (the
/// indented `but ...` block produced by [`equivalent_body`]), as opposed to a
/// generic "syntax has changed" pointer.
pub(crate) fn hint_is_concrete(hint: &str) -> bool {
    hint.contains("\n    but ")
}

/// Hint body naming the concrete modern equivalent of a retired invocation.
fn equivalent_body(cmd: &str, modern: &str) -> String {
    format!(
        "this invocation used retired `but {cmd}` syntax. The modern equivalent is:\n\n    \
         {modern}\n\n\
         See `but {cmd} --help` for details."
    )
}

/// Hint body for retired syntax where no concrete rewrite can be derived.
fn changed_body(cmd: &str) -> String {
    format!("`but {cmd}` syntax has changed. See `but {cmd} --help` for the current form.")
}

/// Whether the tail contains one of the given retired-only flags, in bare or
/// attached-value form. This is what separates retired syntax from a modern
/// invocation that merely failed to parse; hints must never fire for the
/// latter.
fn has_retired_flag(tail: &[OsString], flags: &[&str]) -> bool {
    tail.iter().filter_map(|token| token.to_str()).any(|token| {
        flags.iter().any(|flag| match token.strip_prefix(flag) {
            // Long flags attach a value with `=`; short flags attach it
            // directly.
            Some(rest) => rest.is_empty() || !flag.starts_with("--") || rest.starts_with('='),
            None => false,
        })
    })
}

/// Whether a captured value can be re-emitted verbatim inside a suggested
/// command line: nothing flag-like, empty, or containing characters that
/// would change shell tokenization when the suggestion is copy-pasted.
/// Anything else falls back to a hint without a concrete rewrite.
pub(crate) fn plain(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | '#'))
}

/// Parse a subcommand tail under a retired grammar, for deriving suggestions.
fn parse_retired<P: clap::Parser>(tail: &[OsString]) -> Option<P> {
    let parse = std::iter::once(OsString::from("but")).chain(tail.iter().cloned());
    P::try_parse_from(parse).ok()
}

/// The retired `but amend` grammar: `but amend <commit> --changes <id>,<id>`.
#[derive(Debug, clap::Parser)]
struct RetiredAmend {
    /// The commit to amend into.
    commit: Option<String>,
    #[clap(long = "changes", short = 'p', value_delimiter = ',')]
    changes: Vec<String>,
    #[clap(flatten)]
    globals: RetiredGlobals,
}

/// Retired amend failures: the `--changes` flag is the only marker. Bare
/// positionals are left to the plain parse error — `but amend <a> <b>` is
/// just as likely a modern invocation missing `--target`, and a retired
/// reading would reverse it.
fn amend_hint(tail: &[OsString], agent: bool) -> Option<String> {
    if !has_retired_flag(tail, &["--changes", "-p"]) {
        return None;
    }
    let modern = parse_retired::<RetiredAmend>(tail).and_then(|retired| {
        let commit = retired.commit?;
        (plain(&commit) && !retired.changes.is_empty() && retired.changes.iter().all(|c| plain(c)))
            .then(|| format!("but amend -t {commit} {}", retired.changes.join(" ")))
    });
    Some(match modern {
        Some(modern) => note(&equivalent_body("amend", &modern), agent),
        None => note(&changed_body("amend"), agent),
    })
}

/// The retired `but move` grammar: `but move <source> <target> [--after]`,
/// with comma-separated multi-commit sources and the uncommitted-area selector as the unstack
/// target. Both retired `zz` and current `@` spellings receive the migration hint.
#[derive(Debug, clap::Parser)]
struct RetiredMove {
    source: Option<String>,
    target: Option<String>,
    #[clap(short = 'a', long)]
    after: bool,
    #[clap(flatten)]
    globals: RetiredGlobals,
}

/// Retired move failures: the two-positional `but move <source> <target>`
/// form (the modern grammar requires a placement flag), or the retired
/// `--after` flag.
fn move_hint(tail: &[OsString], agent: bool) -> Option<String> {
    let body = parse_retired::<RetiredMove>(tail).and_then(|retired| {
        let (Some(source), Some(target)) = (retired.source, retired.target) else {
            return None;
        };
        let sources: Vec<&str> = source.split(',').collect();
        let unstack_target = target == "zz" || target == "@";
        if (!unstack_target && !plain(&target)) || sources.iter().any(|source| !plain(source)) {
            return None;
        }
        let sources = sources.join(" ");
        if unstack_target {
            // An uncommitted-area target tore a branch off its stack.
            return (!retired.after)
                .then(|| equivalent_body("move", &format!("but move {sources} --unstack")));
        }
        if retired.after {
            // `--after` only applied to commit targets, where it meant above.
            return Some(equivalent_body(
                "move",
                &format!("but move {sources} --above {target}"),
            ));
        }
        // The target's kind is unknown without resolving it, and the modern
        // placement differs: the retired default put sources before (below) a
        // commit target, but placed them on top of a branch target.
        Some(equivalent_body(
            "move",
            &format!(
                "but move {sources} --below {target}     if {target} is a commit\n    \
                 but move {sources} --branch {target}    if {target} is a branch\n    \
                 but move {sources} --above {target}     to stack a branch onto branch {target}"
            ),
        ))
    });
    match body {
        Some(body) => Some(note(&body, agent)),
        None => {
            has_retired_flag(tail, &["--after", "-a"]).then(|| note(&changed_body("move"), agent))
        }
    }
}

/// Retired squash failures: only the retired-only flags, since the retired
/// positional forms parse under the modern grammar and are hinted at resolve
/// time instead.
fn squash_hint(tail: &[OsString], agent: bool) -> Option<String> {
    if has_retired_flag(tail, &["--drop-message", "-d"]) {
        return Some(note(
            "the retired `--drop-message` flag is now `--use-target-message` (`-u`). \
             See `but squash --help` for details.",
            agent,
        ));
    }
    has_retired_flag(tail, &["--ai", "-i"]).then(|| note(&changed_body("squash"), agent))
}

/// The retired `but uncommit` grammar, for extracting the single source when
/// suggesting `but discard`.
#[derive(Debug, clap::Parser)]
struct RetiredUncommit {
    source: Option<String>,
    #[clap(long, short = 'd')]
    discard: bool,
    #[clap(long)]
    diff: bool,
    #[clap(flatten)]
    globals: RetiredGlobals,
}

/// The modern `but uncommit <source>...` keeps the retired shape, so only the
/// retired-only flags get a hint; other failures return `None`.
fn uncommit_hint(tail: &[OsString], agent: bool) -> Option<String> {
    if has_retired_flag(tail, &["--discard", "-d"]) {
        let modern = parse_retired::<RetiredUncommit>(tail)
            .and_then(|retired| retired.source)
            .filter(|source| plain(source))
            .map_or_else(
                || "but discard <source>...".to_owned(),
                |source| format!("but discard {source}"),
            );
        return Some(note(
            &format!(
                "the retired `but uncommit --discard` is now its own command:\n\n    \
                 {modern}\n\n\
                 See `but discard --help` for details."
            ),
            agent,
        ));
    }
    has_retired_flag(tail, &["--diff"]).then(|| {
        note(
            "the retired `--diff` flag was removed; run `but diff` after uncommitting \
             to see the result.",
            agent,
        )
    })
}

#[cfg(all(test, feature = "legacy"))]
mod tests {
    use clap::Parser as _;

    use super::{Translation, hint, translate_commit};
    use crate::args::{Args, Subcommands, commit::Platform};

    fn argv(args: &[&str]) -> Vec<std::ffi::OsString> {
        std::iter::once("but")
            .chain(args.iter().copied())
            .map(Into::into)
            .collect()
    }

    fn translate(args: &[&str]) -> Translation {
        translate_commit(&argv(args))
    }

    fn translated(args: &[&str]) -> Vec<String> {
        match translate(args) {
            Translation::Translated(translated) => translated
                .into_iter()
                .map(|arg| arg.into_string().expect("translated args are UTF-8"))
                .collect(),
            outcome => panic!("expected a translation, got {outcome:?}"),
        }
    }

    /// Translate and parse with the real CLI parser, proving the rewritten
    /// command line is accepted and lands in the expected fields.
    fn translate_and_parse(args: &[&str]) -> Platform {
        let args = Args::try_parse_from(translated(args)).expect("translated args parse");
        match args.cmd.expect("a subcommand") {
            Subcommands::Commit(platform) => platform,
            cmd => panic!("expected commit command, got {cmd:?}"),
        }
    }

    fn change_names(changes: Vec<crate::args::atoms::CliIdArg>) -> Vec<String> {
        changes.into_iter().map(|change| change.0).collect()
    }

    #[test]
    fn full_retired_form() {
        let platform = translate_and_parse(&[
            "commit",
            "my-branch",
            "-c",
            "-m",
            "msg",
            "--changes",
            "ab,cd",
        ]);
        let branch = platform.branch.expect("branch flag set").expect("named");
        assert_eq!(branch.0, "my-branch");
        assert_eq!(platform.message, Some(vec!["msg".to_owned()]));
        assert_eq!(change_names(platform.changes), ["ab", "cd"]);
    }

    #[test]
    fn changes_only_without_branch() {
        let platform = translate_and_parse(&["commit", "--changes=ab", "-m", "msg"]);
        assert!(platform.branch.is_none(), "no branch flag without -c");
        assert_eq!(change_names(platform.changes), ["ab"]);
    }

    #[test]
    fn create_without_branch_name_requests_generated_branch() {
        let platform = translate_and_parse(&["commit", "-c", "-m", "msg"]);
        assert!(
            matches!(platform.branch, Some(None)),
            "bare -c maps to a generated-name branch"
        );
    }

    #[test]
    fn all_flag_is_dropped() {
        let platform = translate_and_parse(&["commit", "my-branch", "-a", "-m", "msg"]);
        let branch = platform.branch.expect("branch flag set").expect("named");
        assert_eq!(branch.0, "my-branch");
        assert!(platform.changes.is_empty());
    }

    #[test]
    fn short_changes_flag_with_attached_value() {
        let platform = translate_and_parse(&["commit", "my-branch", "-c", "-pab,cd", "-m", "msg"]);
        assert_eq!(change_names(platform.changes), ["ab", "cd"]);
    }

    #[test]
    fn bundled_short_flags() {
        // The `git commit -am`-style habit the retired parser accepted.
        let platform = translate_and_parse(&["commit", "-am", "msg"]);
        assert_eq!(platform.message, Some(vec!["msg".to_owned()]));
        assert!(platform.branch.is_none());

        let platform = translate_and_parse(&["commit", "my-branch", "-cm", "msg"]);
        let branch = platform.branch.expect("branch flag set").expect("named");
        assert_eq!(branch.0, "my-branch");
        assert_eq!(platform.message, Some(vec!["msg".to_owned()]));

        let platform = translate_and_parse(&["commit", "-cpab,cd", "-m", "msg"]);
        assert!(
            matches!(platform.branch, Some(None)),
            "bundled -c without a branch name maps to a generated-name branch"
        );
        assert_eq!(change_names(platform.changes), ["ab", "cd"]);

        let platform = translate_and_parse(&["commit", "-camsg-text"]);
        assert_eq!(platform.message, Some(vec!["sg-text".to_owned()]));
    }

    #[test]
    fn unknown_flags_reject_translation() {
        assert_eq!(
            translate(&["commit", "-ax", "-m", "msg"]),
            Translation::NotRetired
        );
        assert_eq!(
            translate(&["commit", "-cb", "-m", "msg"]),
            Translation::NotRetired
        );
    }

    #[test]
    fn empty_changes_components_refuse_translation() {
        // The retired binary rejected empty components during ID resolution;
        // dropping them could silently commit less (or, for a fully empty
        // list, everything uncommitted), so refuse to translate.
        for retired in [
            &["commit", "b", "-c", "-m", "msg", "--changes", ""][..],
            &["commit", "b", "-c", "-m", "msg", "--changes="],
            &["commit", "b", "-c", "-m", "msg", "--changes", ",,"],
            &["commit", "b", "-c", "-m", "msg", "--changes", "one,"],
            &["commit", "b", "-c", "-m", "msg", "--changes", "one,,two"],
        ] {
            assert_eq!(translate(retired), Translation::Refused, "{retired:?}");
        }
    }

    #[test]
    fn flag_looking_changes_values_do_not_translate() {
        // A `--changes` value that looks like a flag must never be re-emitted
        // as a positional (clap would re-parse it as a flag and commit
        // everything). Attached forms are refused; a missing value fails the
        // retired parse outright, like the retired binary did.
        assert_eq!(
            translate(&["commit", "b", "-c", "-m", "msg", "--changes=-x"]),
            Translation::Refused
        );
        for retired in [
            &["commit", "b", "-c", "-m", "msg", "--changes", "--json"][..],
            &["commit", "b", "-cp", "--json", "-m", "msg"],
            &["commit", "b", "-c", "-m", "msg", "--changes"],
        ] {
            assert_eq!(translate(retired), Translation::NotRetired, "{retired:?}");
        }
    }

    #[test]
    fn global_flags_after_subcommand_are_preserved() {
        assert_eq!(
            translated(&[
                "commit",
                "b",
                "-c",
                "-m",
                "msg",
                "--changes",
                "ab",
                "--json"
            ]),
            [
                "but",
                "commit",
                "--message=msg",
                "--json",
                "--branch=b",
                "ab"
            ]
        );
    }

    #[test]
    fn trailing_positionals_reject_translation() {
        // The retired grammar had a single positional; anything after `--` was
        // a second one and errored.
        assert_eq!(
            translate(&[
                "commit",
                "b",
                "-c",
                "-m",
                "msg",
                "--changes",
                "ab",
                "--",
                "two.txt"
            ]),
            Translation::NotRetired
        );
    }

    #[test]
    fn message_value_is_not_mistaken_for_branch() {
        let platform = translate_and_parse(&[
            "commit",
            "-c",
            "-m",
            "looks-like-a-branch",
            "--changes",
            "ab",
        ]);
        assert!(
            matches!(platform.branch, Some(None)),
            "the -m value must not become the branch"
        );
        assert_eq!(change_names(platform.changes), ["ab"]);
    }

    #[test]
    fn root_options_are_preserved() {
        assert_eq!(
            translated(&["-C", "/repo", "commit", "my-branch", "-c", "-m", "msg"]),
            [
                "but",
                "-C",
                "/repo",
                "commit",
                "--message=msg",
                "--branch=my-branch"
            ]
        );
    }

    #[test]
    fn modern_syntax_is_not_translated() {
        assert_eq!(
            translate(&["commit", "ab", "cd", "-b", "my-branch", "-m", "msg"]),
            Translation::NotRetired
        );
        assert_eq!(
            translate(&["commit", "--no-message"]),
            Translation::NotRetired
        );
    }

    #[test]
    fn other_subcommands_are_not_translated() {
        assert_eq!(
            translate(&["amend", "-c", "--changes", "ab"]),
            Translation::NotRetired
        );
        assert_eq!(translate(&["branch", "new", "-c"]), Translation::NotRetired);
    }

    #[test]
    fn retired_flags_without_modern_equivalent_are_refused() {
        for retired in [
            &["commit", "my-branch", "-c", "--no-hooks", "-m", "msg"][..],
            &["commit", "my-branch", "-c", "-m", "msg", "--before", "ab"],
            &["commit", "my-branch", "--changes", "ab", "--ai"],
            &["commit", "my-branch", "--changes", "ab", "-i=prompt"],
            &["commit", "my-branch", "-c", "--diff"],
        ] {
            assert_eq!(translate(retired), Translation::Refused, "{retired:?}");
        }
    }

    #[test]
    fn hyphen_leading_message_still_binds() {
        // The retired parser accepted `--message=-hello`; the attached form
        // keeps binding it through translation.
        let platform = translate_and_parse(&["commit", "b", "-c", "--message=-hello"]);
        assert_eq!(platform.message, Some(vec!["-hello".to_owned()]));
    }

    #[test]
    fn skill_pointer_only_shown_to_agents() {
        assert!(hint(true).contains("but skill install"));
        assert!(!hint(false).contains("but skill install"));
        let retired_amend = failure_hint(&["amend", "j4", "--changes", "ab"], true);
        assert!(retired_amend.expect("a hint").contains("but skill install"));
    }

    fn failure_hint(args: &[&str], agent: bool) -> Option<String> {
        super::parse_failure_hint(&argv(args), agent).map(|(_, hint)| hint)
    }

    /// The concrete modern equivalent suggested for a retired command line.
    #[track_caller]
    fn suggested(args: &[&str]) -> String {
        let hint = failure_hint(args, false).expect("a hint");
        let (_, tail) = hint
            .split_once("    but ")
            .unwrap_or_else(|| panic!("no suggested command line in {hint:?}"));
        let (modern, _) = tail.split_once('\n').expect("suggestion ends the line");
        format!("but {modern}")
    }

    #[track_caller]
    fn assert_generic(args: &[&str], cmd: &str) {
        let hint = failure_hint(args, false).expect("a hint");
        assert!(
            hint.contains(&format!("`but {cmd}` syntax has changed")),
            "expected the generic changed-syntax hint, got {hint:?}"
        );
    }

    #[test]
    fn retired_amend_changes_form_suggests_the_target_flag() {
        assert_eq!(
            suggested(&["amend", "j4", "--changes", "ab,cd"]),
            "but amend -t j4 ab cd"
        );
        assert_eq!(suggested(&["amend", "j4", "-pab"]), "but amend -t j4 ab");
    }

    #[test]
    fn amend_changes_flag_without_rewrite_gets_the_generic_hint() {
        // Values that would be re-parsed as flags are never re-emitted, but
        // `--changes` itself is a retired-only marker.
        assert_generic(&["amend", "j4", "--changes=-x"], "amend");
        assert_generic(&["amend", "j4", "--changes=one,"], "amend");
        // Values that would change shell tokenization if the suggestion is
        // copy-pasted are never re-emitted either.
        assert_generic(&["amend", "j4", "--changes", "a;echo"], "amend");
        assert_generic(&["amend", "j4;echo", "--changes", "ab"], "amend");
    }

    #[test]
    fn modern_amend_mistakes_get_no_hint() {
        // A single positional without --changes was an error in the retired
        // grammar too; a hint would misdiagnose a modern mistake.
        assert_eq!(failure_hint(&["amend", "j4"], false), None);
        assert_eq!(failure_hint(&["amend", "--bogus"], false), None);
        // Bare positionals are as likely a modern invocation missing
        // `--target` as the retired hidden two-positional form; a retired
        // reading would reverse source and target.
        assert_eq!(failure_hint(&["amend", "ab", "j4"], false), None);
        assert_eq!(failure_hint(&["amend", "ab", "cd", "ef"], false), None);
    }

    #[test]
    fn retired_move_forms_suggest_placement_flags() {
        // The retired default placement depended on the target's kind, which
        // cannot be resolved at parse time, so both readings are offered.
        let hint = failure_hint(&["move", "ab", "cd"], false).expect("a hint");
        assert!(hint.contains("but move ab --below cd"), "got {hint:?}");
        assert!(hint.contains("but move ab --branch cd"), "got {hint:?}");
        // Comma-separated multi-commit sources become positionals.
        let hint = failure_hint(&["move", "ab,cd", "ef"], false).expect("a hint");
        assert!(hint.contains("but move ab cd --below ef"), "got {hint:?}");
        // `--after` only applied to commit targets, and an uncommitted-area target tore
        // a branch off; both map to a single modern equivalent.
        assert_eq!(
            suggested(&["move", "ab", "cd", "--after"]),
            "but move ab --above cd"
        );
        assert_eq!(suggested(&["move", "ab", "@"]), "but move ab --unstack");
        // Preserve teaching hint for invocations written before `@` replaced `zz`.
        assert_eq!(suggested(&["move", "ab", "zz"]), "but move ab --unstack");
    }

    #[test]
    fn retired_move_flag_without_rewrite_gets_the_generic_hint() {
        // `--after` never applied to the unstack target, so there is no
        // equivalent to suggest — but the flag itself is a retired-only
        // marker.
        assert_generic(&["move", "ab", "@", "--after"], "move");
        assert_generic(&["move", "ab", "zz", "--after"], "move");
    }

    #[test]
    fn modern_move_mistakes_get_no_hint() {
        assert_eq!(failure_hint(&["move", "ab"], false), None);
        assert_eq!(failure_hint(&["move", "ab,", "cd"], false), None);
        assert_eq!(failure_hint(&["move", "-b"], false), None);
        // Modern long flags must not be mistaken for the retired short `-a`.
        assert_eq!(
            failure_hint(&["move", "--below", "x", "--above", "y"], false),
            None
        );
    }

    #[test]
    fn retired_squash_flags_get_hints_and_other_failures_none() {
        let hint = failure_hint(&["squash", "ab", "cd", "-d"], false).expect("a hint");
        assert!(hint.contains("--use-target-message"), "got {hint:?}");
        assert_generic(&["squash", "ab", "--ai"], "squash");
        assert_generic(&["squash", "ab", "--ai=summary"], "squash");
        // Modern mistakes are left to the plain parse error.
        assert_eq!(failure_hint(&["squash"], false), None);
        assert_eq!(failure_hint(&["squash", "--bogus"], false), None);
    }

    #[test]
    fn retired_uncommit_flags_get_hints_and_other_failures_none() {
        let hint = failure_hint(&["uncommit", "--discard", "ab"], false).expect("a hint");
        assert!(hint.contains("but discard ab"), "got {hint:?}");
        let hint = failure_hint(&["uncommit", "-d"], false).expect("a hint");
        assert!(hint.contains("but discard <source>..."), "got {hint:?}");
        let hint = failure_hint(&["uncommit", "--discard=true", "ab"], false).expect("a hint");
        assert!(hint.contains("but discard"), "got {hint:?}");
        let hint = failure_hint(&["uncommit", "--diff", "ab"], false).expect("a hint");
        assert!(hint.contains("but diff"), "got {hint:?}");
        // The modern grammar kept the retired shape, so any other failure is
        // not retired syntax.
        assert_eq!(failure_hint(&["uncommit", "--bogus", "ab"], false), None);
    }

    #[test]
    fn other_subcommands_get_no_failure_hint() {
        assert_eq!(failure_hint(&["branch", "--bogus"], false), None);
        assert_eq!(failure_hint(&["discard"], false), None);
    }

    #[test]
    fn failure_hints_skip_root_options() {
        assert_eq!(
            suggested(&["-C", "/repo", "move", "ab", "cd", "--after"]),
            "but move ab --above cd"
        );
    }

    #[test]
    fn rub_hint_teaches_squash_and_move() {
        let hint = super::removed_command_hint(std::ffi::OsStr::new("rub"), false)
            .expect("rub was removed by the revamp");
        assert!(hint.contains("but squash <source>... -t <target>"));
        assert!(hint.contains("placement flag"));
        let other = super::removed_command_hint(std::ffi::OsStr::new("frobnicate"), false);
        assert_eq!(other, None, "only removed commands get a hint");
    }
}
