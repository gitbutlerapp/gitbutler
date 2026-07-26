# Editing the bundled `but` skill

`SKILL.md` and `references/` are installed verbatim into users' agents — a wrong
line here misdirects every agent in every user repo. Only the files in
`SKILL_FILES` in `crates/but/src/command/skill/mod.rs` ship, so register any new
reference file there.

- **Never document anything that can block on a TTY.** An editor or interactive
  picker hangs an agent forever. Give the non-interactive form (`-m`,
  `--no-message`, `-F`, `-t`, `--yes`), and where the blocking variant is one
  omitted argument away — bare `but push` — name it and warn against it rather
  than staying silent.
- **Never document what you have not observed.** Help text and doc comments
  state intent and drift from behavior, so build the CLI and run the command
  against a scratch repo — setting `E2E_TEST_APP_DATA_DIR` to a temp dir keeps
  it off your real GitButler data. Sample output in examples is a claim too.
  Warnings need the same evidence: if you cannot reproduce the failure one
  prevents, cut it.
- **Never document commands agents should not run:** subcommands and flags
  marked `hide = true` in `crates/but/src/args/` (the hidden flags on `push` are
  the usual trap), and the TUI/GUI surfaces.
- **When a command changes, re-derive its guidance** instead of syntax-swapping
  the prose; rationale written for the old implementation dies with it. The same
  facts are deliberately repeated across the four installed files — grep and
  update every occurrence.
- **Leave `version: 0.0.0` alone.** `inject_version` string-replaces that exact
  text at install time, so "fixing" it silently breaks skill versioning.
- **Keep the frontmatter `description` under 1024 characters.** Past that, Codex
  drops it outright and Claude Code truncates it — the skill loses its trigger
  text with no error either way.
