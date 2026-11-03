//! Notes for common words that aren't `but` commands, which people and agents try out of habit
//! from Git and from older versions of `but`. Clap can only offer the nearest spelling, which
//! turns `stage` into `status` and `add` into `land`.
//!
//! Aliases and `but-<word>` executables run first, so a note only shows when nothing else would.

use std::ffi::{OsStr, OsString};

use crate::{args::find_subcommand, theme::Paint as _};

const BRANCH_LIST: &str = "List branches with `but branch list --all`, `but branch list --local` \
                           or `but branch list --remote`.";
const RENAME: &str = "Rename an applied branch with:\n\n    but reword <branch> -m <new-name>\n";
const STAGE: &str = "`but` has no staging area, so name the changes when you commit them:\n\n    \
                     but commit -b <branch> -m \"message\" <file-or-id>...\n";
const SWITCH: &str = "To check out one branch on its own, use `but switch <branch>`, and \
                      `but switch --workspace` to go back to all applied branches. \
                      `but apply <branch>` adds a branch to the applied ones, and \
                      `but branch new <name>` creates one. To drop a file's uncommitted changes, \
                      use `but discard <file-or-id>`.";
const TARGET: &str = "`but config target` shows the target branch and `but config target <branch>` \
                      sets it. `but pull --check` lists its new commits and `but pull` rebases \
                      onto them.";

/// The words after `but` that get a note, and what the note says to do instead.
const NOTES: &[(&str, &str)] = &[
    ("add", STAGE),
    ("base", TARGET),
    ("branch --all", BRANCH_LIST),
    ("branch --list", BRANCH_LIST),
    ("branch --remote", BRANCH_LIST),
    ("branch -a", BRANCH_LIST),
    ("branch -m", RENAME),
    ("branch -r", BRANCH_LIST),
    ("branch checkout", SWITCH),
    (
        "branch create",
        "Create a branch with:\n\n    but branch new <name>\n",
    ),
    (
        "branch current",
        "`but status` lists the applied branches and marks a checked-out branch with `[HEAD]`.",
    ),
    ("branch rename", RENAME),
    ("branch switch", SWITCH),
    ("checkout", SWITCH),
    (
        "log",
        "`but status` lists the commits on applied branches, `but branch show <branch>` the \
         commits of one branch, and `but show <commit>` a single commit. For older history, use \
         `git log`.",
    ),
    (
        "rebase",
        "`but pull` rebases applied branches onto the latest target. To edit history by hand, use \
         `but move <commit> --below <commit>`, `but squash <commit> -t <commit>` or \
         `but reword <commit> -m \"message\"`.",
    ),
    ("stage", STAGE),
    (
        "stash",
        "`but pull` keeps uncommitted changes, and `but pull --check` warns when they may \
         conflict. To park them, commit them on their own branch and unapply it:\n\n    \
         but commit -b <new-branch> -m \"message\" <file-or-id>...\n    \
         but unapply <new-branch>\n\n\
         `but apply <new-branch>` brings them back as a commit.",
    ),
    ("target", TARGET),
];

/// The note for `but <word>` when no alias or executable provides `word`.
pub(crate) fn external_command_note(word: &OsStr, agent: bool) -> Option<String> {
    note_for(word.to_str()?, agent)
}

/// The note for a command line the parser rejected at the word after its subcommand.
pub(crate) fn parse_failure_note(args: &[OsString], agent: bool) -> Option<String> {
    let (ix, subcommand) = find_subcommand(args)?;
    let word = args.get(ix + 1)?;
    note_for(
        &format!("{} {}", subcommand.to_str()?, word.to_str()?),
        agent,
    )
}

fn note_for(words: &str, agent: bool) -> Option<String> {
    let (_, body) = NOTES.iter().find(|(name, _)| *name == words)?;
    Some(note(&format!("`but {words}` doesn't exist. {body}"), agent))
}

/// A stderr note in the shared teaching-hint format, with a pointer to the
/// bundled skill docs when an agent is driving the CLI.
pub(crate) fn note(body: &str, agent: bool) -> String {
    let t = crate::theme::get();
    let mut text = format!("\n{} {body}\n", t.attention.paint("note:"));
    if agent {
        text.push_str(
            "Run `but skill install` (or `but skill check --update`) to load current command docs.\n",
        );
    }
    text
}

#[cfg(all(test, feature = "legacy"))]
mod tests {
    use clap::Parser as _;

    use super::NOTES;
    use crate::args::{Args, Subcommands};

    /// Parse a command line as a note writes it, with its placeholders filled in.
    fn parse(command: &str) -> Result<Args, clap::Error> {
        Args::try_parse_from(
            command
                .split_whitespace()
                .map(|word| if word.starts_with('<') { "x" } else { word }),
        )
    }

    fn is_external(args: &Args) -> bool {
        matches!(args.cmd, Some(Subcommands::External(_)))
    }

    #[test]
    fn every_entry_is_a_command_line_the_parser_does_not_run() {
        for (words, _) in NOTES {
            let unknown = parse(&format!("but {words}")).map_or(true, |args| is_external(&args));
            assert!(unknown, "`but {words}` runs now, so its note never shows");
        }
    }

    #[test]
    fn every_command_a_note_names_parses() {
        for (_, body) in NOTES {
            let inline = body.split('`').skip(1).step_by(2);
            let blocks = body.lines().filter_map(|line| line.strip_prefix("    "));
            for command in inline.chain(blocks).filter(|text| text.starts_with("but ")) {
                let args = parse(command).unwrap_or_else(|err| panic!("`{command}`: {err}"));
                assert!(!is_external(&args), "`{command}` is not a command");
            }
        }
    }
}
