//! Temporary translation of retired `but commit` syntax into the modern form.
//!
//! Before the July 2026 command revamp, `but commit` was invoked as
//! `but commit <branch> -c -m "message" --changes <id>,<id>`. That form is
//! widespread in LLM training data, so agents keep producing it. Instead of a
//! hard parse error, the retired form is rewritten into the modern
//! `but commit -b <branch> -m "message" <change>...` equivalent, the command
//! runs normally, and a hint teaching the new syntax is printed at the end.
//!
//! The retired grammar is redeclared below as a clap struct, so command lines
//! are tokenized exactly as the pre-revamp binary did (short-flag bundles,
//! `=`-attached values, comma-separated `--changes`). Translation then maps
//! typed fields to modern argv. It only runs as a fallback after the modern
//! parser has rejected the command line, so current syntax is never affected.
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
//! `retiredCommitSyntax` command prop shows the old syntax has aged out of
//! common use.

use std::ffi::OsString;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::theme::Paint as _;

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
    let t = crate::theme::get();
    let mut text = format!(
        "\n{} this invocation used retired `but commit` syntax. The modern form is:\n\n    \
         but commit -b <branch> -m \"message\" <change>...\n\n\
         See `but commit --help` for details.\n",
        t.attention.paint("note:"),
    );
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
    /// Global option in both grammars; may legally follow the subcommand.
    #[clap(long)]
    json: bool,
    /// Global option in both grammars; may legally follow the subcommand.
    #[clap(long)]
    status_after: bool,
}

/// Rewrite a retired `but commit` command line into the modern equivalent.
///
/// Returns [`Translation::NotRetired`] unless the tail after the `commit`
/// subcommand parses under the retired grammar and contains at least one
/// retired-only token.
pub(crate) fn translate_commit(args: &[OsString]) -> Translation {
    let Some(commit_ix) = find_commit_subcommand(args) else {
        return Translation::NotRetired;
    };
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
        json,
        status_after,
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

/// Find the index of the `commit` subcommand token, skipping root options.
fn find_commit_subcommand(args: &[OsString]) -> Option<usize> {
    let mut ix = 1;
    while ix < args.len() {
        let token = &args[ix];
        // Root options that consume a separate value token.
        if token == "-C" || token == "--current-dir" || token == "--log-file" {
            ix += 2;
            continue;
        }
        if token.as_encoded_bytes().starts_with(b"-") {
            ix += 1;
            continue;
        }
        return (token == "commit").then_some(ix);
    }
    None
}

#[cfg(all(test, feature = "legacy"))]
mod tests {
    use clap::Parser as _;

    use super::{Translation, hint, translate_commit};
    use crate::args::{Args, Subcommands, commit::Platform};

    fn translate(args: &[&str]) -> Translation {
        let args: Vec<_> = std::iter::once("but")
            .chain(args.iter().copied())
            .map(Into::into)
            .collect();
        translate_commit(&args)
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
    }
}
